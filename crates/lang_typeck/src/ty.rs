//! Type representation and unification.
//!
//! Types are immutable and reference counted. Record types and effect sets
//! are rows: chains of `RowExt(label, ty, rest)` ending in `RowEmpty` (closed),
//! a variable (open), or a skolem (rigid). Unification is structural with an
//! occurs check, level-based generalization (Rémy levels), and a skolem escape
//! check that implements region (scope brand) confinement.

use std::fmt::Write as _;
use std::rc::Rc;

use ena::unify::{InPlaceUnificationTable, UnifyKey, UnifyValue};
use lang_base::Symbol;
use lang_hir::DefId;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TyVid(u32);

impl UnifyKey for TyVid {
    type Value = VarValue;
    fn index(&self) -> u32 {
        self.0
    }
    fn from_index(i: u32) -> Self {
        TyVid(i)
    }
    fn tag() -> &'static str {
        "TyVid"
    }
}

#[derive(Debug, Clone)]
pub enum VarValue {
    Unbound { level: u32 },
    Bound(Ty),
}

impl UnifyValue for VarValue {
    type Error = ena::unify::NoError;
    fn unify_values(a: &Self, b: &Self) -> Result<Self, Self::Error> {
        Ok(match (a, b) {
            (VarValue::Unbound { level: x }, VarValue::Unbound { level: y }) => {
                VarValue::Unbound { level: *x.min(y) }
            }
            (VarValue::Bound(t), _) | (_, VarValue::Bound(t)) => VarValue::Bound(t.clone()),
        })
    }
}

pub type Ty = Rc<TyKind>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TyKind {
    Var(TyVid),
    /// Nominal type constructor: struct, enum, or opaque/builtin type.
    Con(DefId, Vec<Ty>),
    Fn(FnTy),
    /// A record; the payload is a row.
    Record(Ty),
    Tuple(Vec<Ty>),
    RowEmpty,
    RowExt(Symbol, Ty, Ty),
    Ref(bool, Ty),
    /// Rigid type variable (declared generic inside its definition, or a
    /// region/scope brand).
    Skolem(u32),
    /// Quantified variable of a type scheme.
    Gen(u32),
    /// `dyn Protocol<args>`: a dictionary-dispatched value.
    Dyn(DefId, Vec<Ty>),
    /// `for<S> T`: binders are `Bound(0..n)` inside the body.
    Forall(u32, Ty),
    Bound(u32),
    /// Associated type projection `base.Name`; normalized through the
    /// implementation of `base` once it is known (see `set_assoc_table`).
    Proj(Ty, Symbol),
    /// Poison type after an error; unifies with anything.
    Error,
}

/// One `type Name = T` binding of an implementation; `self_ty` and `ty` use
/// `Gen(i)` for the implementation's generic parameters.
#[derive(Debug, Clone)]
pub struct AssocImpl {
    pub n_generics: u32,
    pub self_ty: Ty,
    pub name: Symbol,
    pub ty: Ty,
}

thread_local! {
    static ASSOC: std::cell::RefCell<Rc<Vec<AssocImpl>>> = std::cell::RefCell::new(Rc::new(vec![]));
}

/// Install the program's associated type bindings (one program is checked
/// and compiled per thread at a time).
pub fn set_assoc_table(t: Vec<AssocImpl>) {
    ASSOC.with(|a| *a.borrow_mut() = Rc::new(t));
}

/// `base.name` for a resolved `base`, if an implementation binds it.
pub fn normalize_proj(base: &Ty, name: Symbol) -> Option<Ty> {
    let table = ASSOC.with(|a| a.borrow().clone());
    let mut base = base.clone();
    while let TyKind::Ref(_, x) = &*base {
        base = x.clone();
    }
    for e in table.iter().filter(|e| e.name == name) {
        let mut binds: Vec<Option<Ty>> = vec![None; e.n_generics as usize];
        if match_pattern(&e.self_ty, &base, &mut binds) {
            let args: Vec<Ty> = binds.into_iter().map(|b| b.unwrap_or_else(error_ty)).collect();
            return Some(crate::env::subst_gen(&e.ty, &args));
        }
    }
    None
}

/// One-way match of an implementation's self type against a type.
fn match_pattern(pat: &Ty, t: &Ty, binds: &mut Vec<Option<Ty>>) -> bool {
    match (&**pat, &**t) {
        (TyKind::Gen(i), _) => match &binds[*i as usize] {
            Some(b) => b == t,
            None => {
                binds[*i as usize] = Some(t.clone());
                true
            }
        },
        (TyKind::Con(d1, a1), TyKind::Con(d2, a2)) | (TyKind::Dyn(d1, a1), TyKind::Dyn(d2, a2)) => {
            d1 == d2 && a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| match_pattern(x, y, binds))
        }
        (TyKind::Tuple(a1), TyKind::Tuple(a2)) => {
            a1.len() == a2.len() && a1.iter().zip(a2).all(|(x, y)| match_pattern(x, y, binds))
        }
        _ => pat == t,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FnTy {
    pub params: Vec<Ty>,
    pub ret: Ty,
    /// Effect row: labels `Io`, `Wait`, `State`, `Foreign`, and region labels.
    pub eff: Ty,
    pub once: bool,
}

pub fn mk(k: TyKind) -> Ty {
    Rc::new(k)
}

pub fn unit() -> Ty {
    thread_local!(static UNIT: Ty = mk(TyKind::Tuple(vec![])));
    UNIT.with(|u| u.clone())
}

pub fn row_empty() -> Ty {
    thread_local!(static E: Ty = mk(TyKind::RowEmpty));
    E.with(|u| u.clone())
}

pub fn error_ty() -> Ty {
    thread_local!(static E: Ty = mk(TyKind::Error));
    E.with(|u| u.clone())
}

/// Build a row from fields and a tail.
pub fn row(fields: impl IntoIterator<Item = (Symbol, Ty)>, tail: Ty) -> Ty {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.reverse();
    fields.into_iter().fold(tail, |rest, (l, t)| mk(TyKind::RowExt(l, t, rest)))
}

pub fn record(fields: impl IntoIterator<Item = (Symbol, Ty)>, tail: Ty) -> Ty {
    mk(TyKind::Record(row(fields, tail)))
}

#[derive(Debug, Clone)]
pub enum UnifyError {
    Mismatch(Ty, Ty),
    /// A closed row lacks a label required by the other side.
    MissingField {
        label: Symbol,
        record: Ty,
    },
    /// A rigid row (skolem tail) cannot gain fields.
    RigidRow {
        label: Symbol,
        record: Ty,
    },
    Occurs(Ty, Ty),
    /// A scoped value (skolem) would escape to an outer scope.
    Escape {
        skolem: u32,
        ty: Ty,
    },
    Arity(usize, usize),
}

#[derive(Debug, Clone)]
pub struct SkolemInfo {
    pub name: Symbol,
    pub level: u32,
    /// Region/scope brand (from `for<S>`), as opposed to a declared generic.
    pub region: bool,
}

pub struct InferCtxt {
    table: InPlaceUnificationTable<TyVid>,
    pub level: u32,
    pub skolems: Vec<SkolemInfo>,
    /// Definition id of `Never` (unifies with anything as the bottom type).
    pub never: Option<DefId>,
    /// Ignore effect-row mismatches (instance re-checking for codegen).
    pub lenient_effects: bool,
    /// Equations on projections of not-yet-known types, retried by the solver.
    pub deferred: Vec<(Ty, Ty, lang_base::Span)>,
    snap_deferred: Vec<usize>,
    /// Source location of the unification in progress (for deferred errors).
    pub span_hint: lang_base::Span,
}

impl InferCtxt {
    pub fn new(never: Option<DefId>) -> Self {
        InferCtxt {
            table: InPlaceUnificationTable::new(),
            level: 1,
            skolems: vec![],
            never,
            lenient_effects: false,
            deferred: vec![],
            snap_deferred: vec![],
            span_hint: lang_base::Span::dummy(),
        }
    }

    pub fn fresh(&mut self) -> Ty {
        mk(TyKind::Var(self.table.new_key(VarValue::Unbound { level: self.level })))
    }

    /// A variable that may be bound to region-branded types without an
    /// escape check (impl matching against a subject).
    pub fn fresh_matching(&mut self) -> Ty {
        mk(TyKind::Var(self.table.new_key(VarValue::Unbound { level: u32::MAX / 2 })))
    }

    pub fn fresh_row(&mut self) -> Ty {
        self.fresh()
    }

    pub fn fresh_skolem(&mut self, name: Symbol, region: bool) -> Ty {
        let id = self.skolems.len() as u32;
        self.skolems.push(SkolemInfo { name, level: self.level, region });
        mk(TyKind::Skolem(id))
    }

    pub fn var_level(&mut self, v: TyVid) -> Option<u32> {
        match self.table.probe_value(v) {
            VarValue::Unbound { level } => Some(level),
            VarValue::Bound(_) => None,
        }
    }

    /// Follow variable bindings at the root.
    pub fn shallow(&mut self, t: &Ty) -> Ty {
        match &**t {
            TyKind::Var(v) => match self.table.probe_value(*v) {
                VarValue::Bound(b) => self.shallow(&b),
                VarValue::Unbound { .. } => {
                    let root = self.table.find(*v);
                    if root == *v { t.clone() } else { mk(TyKind::Var(root)) }
                }
            },
            // a projection whose base is now known stands for its binding
            TyKind::Proj(b, n) => {
                let rb = self.resolve(b);
                match normalize_proj(&rb, *n) {
                    Some(x) => self.shallow(&x),
                    None => t.clone(),
                }
            }
            _ => t.clone(),
        }
    }

    /// Fully substitute all bound variables.
    pub fn resolve(&mut self, t: &Ty) -> Ty {
        let t = self.shallow(t);
        match &*t {
            TyKind::Var(_)
            | TyKind::RowEmpty
            | TyKind::Skolem(_)
            | TyKind::Gen(_)
            | TyKind::Bound(_)
            | TyKind::Error => t,
            TyKind::Con(d, args) => mk(TyKind::Con(*d, args.iter().map(|a| self.resolve(a)).collect())),
            TyKind::Dyn(d, args) => mk(TyKind::Dyn(*d, args.iter().map(|a| self.resolve(a)).collect())),
            TyKind::Fn(f) => mk(TyKind::Fn(FnTy {
                params: f.params.iter().map(|p| self.resolve(p)).collect(),
                ret: self.resolve(&f.ret),
                eff: self.resolve(&f.eff),
                once: f.once,
            })),
            TyKind::Record(r) => mk(TyKind::Record(self.resolve(r))),
            TyKind::Tuple(ts) => mk(TyKind::Tuple(ts.iter().map(|a| self.resolve(a)).collect())),
            TyKind::RowExt(l, a, rest) => mk(TyKind::RowExt(*l, self.resolve(a), self.resolve(rest))),
            TyKind::Ref(m, a) => mk(TyKind::Ref(*m, self.resolve(a))),
            TyKind::Forall(n, b) => mk(TyKind::Forall(*n, self.resolve(b))),
            TyKind::Proj(b, n) => {
                let b = self.resolve(b);
                match normalize_proj(&b, *n) {
                    Some(t) => self.resolve(&t),
                    None => mk(TyKind::Proj(b, *n)),
                }
            }
        }
    }

    /// Unify where at least one side is a projection: normalize what can be
    /// normalized; a projection of a still-unknown type is retried later.
    fn unify_proj(&mut self, a: &Ty, b: &Ty) -> Result<(), UnifyError> {
        let norm = |icx: &mut Self, t: &Ty| -> (Ty, bool) {
            if let TyKind::Proj(base, n) = &**t {
                let rb = icx.resolve(base);
                if let Some(x) = normalize_proj(&rb, *n) {
                    return (x, true);
                }
                let pending = matches!(&*rb, TyKind::Var(_));
                return (mk(TyKind::Proj(rb, *n)), pending);
            }
            (t.clone(), false)
        };
        let (na, pa) = norm(self, a);
        let (nb, pb) = norm(self, b);
        let a_proj = matches!(&*na, TyKind::Proj(..));
        let b_proj = matches!(&*nb, TyKind::Proj(..));
        if !a_proj && !b_proj {
            return self.unify(&na, &nb);
        }
        if let (TyKind::Proj(x, n1), TyKind::Proj(y, n2)) = (&*na, &*nb)
            && n1 == n2
            && self.try_unify(x, y)
        {
            return Ok(());
        }
        if (a_proj && pa) || (b_proj && pb) {
            let span = self.span_hint;
            self.deferred.push((na, nb, span));
            return Ok(());
        }
        Err(UnifyError::Mismatch(na, nb))
    }

    /// Split a row into its fields (in order) and its tail.
    pub fn flatten_row(&mut self, r: &Ty) -> (Vec<(Symbol, Ty)>, Ty) {
        let mut fields = vec![];
        let mut cur = self.shallow(r);
        loop {
            match &*cur {
                TyKind::RowExt(l, t, rest) => {
                    fields.push((*l, t.clone()));
                    let next = self.shallow(rest);
                    cur = next;
                }
                _ => return (fields, cur),
            }
        }
    }

    pub fn is_never(&self, t: &Ty) -> bool {
        matches!(&**t, TyKind::Con(d, _) if Some(*d) == self.never)
    }

    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), UnifyError> {
        let a = self.shallow(a);
        let b = self.shallow(b);
        if Rc::ptr_eq(&a, &b) {
            return Ok(());
        }
        match (&*a, &*b) {
            (TyKind::Var(x), TyKind::Var(y)) => {
                if x != y {
                    self.table.unify_var_var(*x, *y).unwrap();
                }
                Ok(())
            }
            (TyKind::Var(x), _) => self.bind(*x, &b),
            (_, TyKind::Var(y)) => self.bind(*y, &a),
            (TyKind::Error, _) | (_, TyKind::Error) => Ok(()),
            (TyKind::Proj(..), _) | (_, TyKind::Proj(..)) => self.unify_proj(&a, &b),
            _ if self.is_never(&a) || self.is_never(&b) => Ok(()),
            (TyKind::Con(c1, a1), TyKind::Con(c2, a2)) if c1 == c2 && a1.len() == a2.len() => {
                for (x, y) in a1.iter().zip(a2) {
                    self.unify(x, y).map_err(|e| self.wrap(e, &a, &b))?;
                }
                Ok(())
            }
            (TyKind::Dyn(c1, a1), TyKind::Dyn(c2, a2)) if c1 == c2 && a1.len() == a2.len() => {
                for (x, y) in a1.iter().zip(a2) {
                    self.unify(x, y).map_err(|e| self.wrap(e, &a, &b))?;
                }
                Ok(())
            }
            (TyKind::Fn(f1), TyKind::Fn(f2)) => {
                if f1.params.len() != f2.params.len() {
                    return Err(UnifyError::Arity(f1.params.len(), f2.params.len()));
                }
                for (x, y) in f1.params.iter().zip(&f2.params) {
                    self.unify(x, y).map_err(|e| self.wrap(e, &a, &b))?;
                }
                self.unify(&f1.ret, &f2.ret).map_err(|e| self.wrap(e, &a, &b))?;
                if self.lenient_effects {
                    let _ = self.unify_rows(&f1.eff, &f2.eff);
                    return Ok(());
                }
                self.unify_rows(&f1.eff, &f2.eff)
            }
            (TyKind::Record(r1), TyKind::Record(r2)) => self.unify_rows(r1, r2),
            (TyKind::Tuple(t1), TyKind::Tuple(t2)) if t1.len() == t2.len() => {
                for (x, y) in t1.iter().zip(t2) {
                    self.unify(x, y).map_err(|e| self.wrap(e, &a, &b))?;
                }
                Ok(())
            }
            (TyKind::RowEmpty | TyKind::RowExt(..), TyKind::RowEmpty | TyKind::RowExt(..)) => {
                self.unify_rows(&a, &b)
            }
            (TyKind::Ref(_, t1), TyKind::Ref(_, t2)) => self.unify(t1, t2),
            // Borrows are transparent for typing; affine rules are enforced
            // by the ownership checker.
            (TyKind::Ref(_, t1), _) => self.unify(t1, &b),
            (_, TyKind::Ref(_, t2)) => self.unify(&a, t2),
            (TyKind::Skolem(x), TyKind::Skolem(y)) if x == y => Ok(()),
            (TyKind::Gen(x), TyKind::Gen(y)) if x == y => Ok(()),
            (TyKind::Bound(x), TyKind::Bound(y)) if x == y => Ok(()),
            (TyKind::Forall(n1, b1), TyKind::Forall(n2, b2)) if n1 == n2 => self.unify(b1, b2),
            _ => Err(UnifyError::Mismatch(a, b)),
        }
    }

    /// Keep the innermost structural error, but report mismatches of whole
    /// types when a nested component disagrees.
    fn wrap(&mut self, e: UnifyError, a: &Ty, b: &Ty) -> UnifyError {
        match e {
            UnifyError::Mismatch(..) => UnifyError::Mismatch(a.clone(), b.clone()),
            e => e,
        }
    }

    pub fn unify_rows(&mut self, r1: &Ty, r2: &Ty) -> Result<(), UnifyError> {
        let (f1, t1) = self.flatten_row(r1);
        let (f2, t2) = self.flatten_row(r2);
        for (l, ty1) in &f1 {
            if let Some((_, ty2)) = f2.iter().find(|(l2, _)| l2 == l) {
                self.unify(ty1, ty2)?;
            }
        }
        let only1: Vec<(Symbol, Ty)> =
            f1.iter().filter(|(l, _)| !f2.iter().any(|(l2, _)| l2 == l)).cloned().collect();
        let only2: Vec<(Symbol, Ty)> =
            f2.iter().filter(|(l, _)| !f1.iter().any(|(l2, _)| l2 == l)).cloned().collect();
        let t1 = self.shallow(&t1);
        let t2 = self.shallow(&t2);
        match (only1.is_empty(), only2.is_empty()) {
            (true, true) => self.unify_tails(&t1, &t2),
            (false, true) => self.extend_tail(&t2, only1, &t1, r2),
            (true, false) => self.extend_tail(&t1, only2, &t2, r1),
            (false, false) => {
                let same = matches!((&*t1, &*t2), (TyKind::Var(a), TyKind::Var(b)) if a == b);
                match (&*t1, &*t2) {
                    (TyKind::Var(_), TyKind::Var(_)) if !same => {
                        let rest = self.fresh_row();
                        self.extend_tail(&t1, only2, &rest, r1)?;
                        self.extend_tail(&t2, only1, &rest, r2)
                    }
                    (TyKind::Var(_), _) | (TyKind::Error, _) => self.extend_tail(&t2, only1, &t1, r2),
                    _ => self.extend_tail(&t1, only2, &t2, r1),
                }
            }
        }
    }

    fn unify_tails(&mut self, t1: &Ty, t2: &Ty) -> Result<(), UnifyError> {
        match (&**t1, &**t2) {
            (TyKind::RowEmpty, TyKind::RowEmpty) => Ok(()),
            _ => self.unify(t1, t2),
        }
    }

    /// Make `tail` (the tail of `whole`) equal to `{extra | rest}`.
    fn extend_tail(
        &mut self,
        tail: &Ty,
        extra: Vec<(Symbol, Ty)>,
        rest: &Ty,
        whole: &Ty,
    ) -> Result<(), UnifyError> {
        match &**tail {
            TyKind::Var(v) => {
                if let TyKind::Var(w) = &**rest
                    && self.table.unioned(*v, *w)
                {
                    let t = row(extra.clone(), rest.clone());
                    return Err(UnifyError::Occurs(tail.clone(), t));
                }
                let t = row(extra, rest.clone());
                self.bind(*v, &t)
            }
            TyKind::Error => Ok(()),
            TyKind::Skolem(_) => {
                let whole = self.resolve(whole);
                Err(UnifyError::RigidRow { label: extra[0].0, record: mk(TyKind::Record(whole)) })
            }
            _ => {
                let whole = self.resolve(whole);
                Err(UnifyError::MissingField { label: extra[0].0, record: mk(TyKind::Record(whole)) })
            }
        }
    }

    fn bind(&mut self, v: TyVid, t: &Ty) -> Result<(), UnifyError> {
        let level = match self.table.probe_value(v) {
            VarValue::Unbound { level } => level,
            VarValue::Bound(b) => return self.unify(&b, t),
        };
        if let Some(e) = self.occurs_and_adjust(v, level, t) {
            return Err(e);
        }
        self.table.union_value(v, VarValue::Bound(t.clone()));
        Ok(())
    }

    /// Occurs check plus level lowering and skolem escape detection.
    fn occurs_and_adjust(&mut self, v: TyVid, level: u32, t: &Ty) -> Option<UnifyError> {
        let t = self.shallow(t);
        match &*t {
            TyKind::Var(w) => {
                if self.table.unioned(v, *w) {
                    return Some(UnifyError::Occurs(mk(TyKind::Var(v)), t.clone()));
                }
                if let VarValue::Unbound { level: wl } = self.table.probe_value(*w)
                    && wl > level
                {
                    self.table.union_value(*w, VarValue::Unbound { level });
                }
                None
            }
            TyKind::Skolem(s) => {
                let info = &self.skolems[*s as usize];
                if info.region && info.level > level {
                    return Some(UnifyError::Escape { skolem: *s, ty: t.clone() });
                }
                None
            }
            TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
                for a in args {
                    if let Some(e) = self.occurs_and_adjust(v, level, a) {
                        return Some(e);
                    }
                }
                None
            }
            TyKind::Fn(f) => {
                for a in f.params.iter().chain([&f.ret, &f.eff]) {
                    if let Some(e) = self.occurs_and_adjust(v, level, a) {
                        return Some(e);
                    }
                }
                None
            }
            TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => {
                self.occurs_and_adjust(v, level, r)
            }
            TyKind::RowExt(_, a, rest) => {
                self.occurs_and_adjust(v, level, a).or_else(|| self.occurs_and_adjust(v, level, rest))
            }
            TyKind::RowEmpty | TyKind::Gen(_) | TyKind::Bound(_) | TyKind::Error => None,
        }
    }

    /// Unbound variables (roots) occurring in `t`, in first-occurrence order.
    pub fn free_vars(&mut self, t: &Ty, out: &mut Vec<TyVid>) {
        let t = self.shallow(t);
        match &*t {
            TyKind::Var(v) => {
                let r = self.table.find(*v);
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
                args.iter().for_each(|a| self.free_vars(a, out))
            }
            TyKind::Fn(f) => {
                f.params.iter().for_each(|a| self.free_vars(a, out));
                self.free_vars(&f.ret, out);
                self.free_vars(&f.eff, out);
            }
            TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => {
                self.free_vars(r, out)
            }
            TyKind::RowExt(_, a, rest) => {
                self.free_vars(a, out);
                self.free_vars(rest, out);
            }
            _ => {}
        }
    }

    /// Free variables outside effect rows: the ones that can change a
    /// value's representation when instantiated.
    pub fn free_value_vars(&mut self, t: &Ty, out: &mut Vec<TyVid>) {
        let t = self.shallow(t);
        match &*t {
            TyKind::Fn(f) => {
                f.params.iter().for_each(|a| self.free_value_vars(a, out));
                self.free_value_vars(&f.ret, out);
            }
            TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
                args.iter().for_each(|a| self.free_value_vars(a, out))
            }
            TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => {
                self.free_value_vars(r, out)
            }
            TyKind::RowExt(_, a, rest) => {
                self.free_value_vars(a, out);
                self.free_value_vars(rest, out);
            }
            _ => self.free_vars(&t, out),
        }
    }

    pub fn contains_skolem(&mut self, t: &Ty, s: u32) -> bool {
        let t = self.resolve(t);
        ty_any(&t, &mut |k| matches!(k, TyKind::Skolem(x) if *x == s))
    }

    /// Unify inside a snapshot: on failure, roll back all bindings.
    pub fn try_unify(&mut self, a: &Ty, b: &Ty) -> bool {
        let snap = self.table.snapshot();
        let nsk = self.skolems.len();
        let nd = self.deferred.len();
        match self.unify(a, b) {
            Ok(()) => {
                self.table.commit(snap);
                true
            }
            Err(_) => {
                self.table.rollback_to(snap);
                self.skolems.truncate(nsk);
                self.deferred.truncate(nd);
                false
            }
        }
    }

    pub fn snapshot(&mut self) -> ena::unify::Snapshot<ena::unify::InPlace<TyVid>> {
        self.snap_deferred.push(self.deferred.len());
        self.table.snapshot()
    }

    pub fn rollback(&mut self, s: ena::unify::Snapshot<ena::unify::InPlace<TyVid>>) {
        if let Some(n) = self.snap_deferred.pop() {
            self.deferred.truncate(n);
        }
        self.table.rollback_to(s);
    }

    pub fn commit(&mut self, s: ena::unify::Snapshot<ena::unify::InPlace<TyVid>>) {
        self.snap_deferred.pop();
        self.table.commit(s);
    }

    pub fn root(&mut self, v: TyVid) -> TyVid {
        self.table.find(v)
    }
}

/// Does any node of `t` satisfy `f`?
pub fn ty_any(t: &Ty, f: &mut impl FnMut(&TyKind) -> bool) -> bool {
    if f(t) {
        return true;
    }
    match &**t {
        TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
            args.iter().any(|a| ty_any(a, f))
        }
        TyKind::Fn(ft) => ft.params.iter().any(|a| ty_any(a, f)) || ty_any(&ft.ret, f) || ty_any(&ft.eff, f),
        TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => ty_any(r, f),
        TyKind::RowExt(_, a, rest) => ty_any(a, f) || ty_any(rest, f),
        _ => false,
    }
}

/// Structural substitution helper: rebuild `t`, replacing nodes for which
/// `f` returns `Some`.
pub fn ty_map(t: &Ty, f: &mut impl FnMut(&Ty) -> Option<Ty>) -> Ty {
    if let Some(r) = f(t) {
        return r;
    }
    match &**t {
        TyKind::Con(d, args) => mk(TyKind::Con(*d, args.iter().map(|a| ty_map(a, f)).collect())),
        TyKind::Tuple(args) => mk(TyKind::Tuple(args.iter().map(|a| ty_map(a, f)).collect())),
        TyKind::Dyn(d, args) => mk(TyKind::Dyn(*d, args.iter().map(|a| ty_map(a, f)).collect())),
        TyKind::Fn(ft) => mk(TyKind::Fn(FnTy {
            params: ft.params.iter().map(|a| ty_map(a, f)).collect(),
            ret: ty_map(&ft.ret, f),
            eff: ty_map(&ft.eff, f),
            once: ft.once,
        })),
        TyKind::Record(r) => mk(TyKind::Record(ty_map(r, f))),
        TyKind::Ref(m, r) => mk(TyKind::Ref(*m, ty_map(r, f))),
        TyKind::Forall(n, r) => mk(TyKind::Forall(*n, ty_map(r, f))),
        TyKind::RowExt(l, a, rest) => mk(TyKind::RowExt(*l, ty_map(a, f), ty_map(rest, f))),
        TyKind::Proj(b, n) => {
            let b = ty_map(b, f);
            // a substituted base may now select an implementation
            normalize_proj(&b, *n).unwrap_or_else(|| mk(TyKind::Proj(b, *n)))
        }
        _ => t.clone(),
    }
}

/// Fields of a fully resolved row, sorted by label text (canonical order).
pub fn row_fields(r: &Ty) -> (Vec<(Symbol, Ty)>, Ty) {
    let mut fields = vec![];
    let mut cur = r.clone();
    loop {
        match &*cur {
            TyKind::RowExt(l, t, rest) => {
                fields.push((*l, t.clone()));
                cur = rest.clone();
            }
            _ => return (fields, cur),
        }
    }
}

// ------------------------------------------------------------------ printing

/// Names used when printing types.
pub trait TyNames {
    fn sym(&self, s: Symbol) -> &str;
    fn def_name(&self, d: DefId) -> String;
    fn skolem_name(&self, s: u32) -> String;
}

pub struct Printer<'a, N: TyNames> {
    pub names: &'a N,
    vars: FxHashMap<TyVid, String>,
    /// Source names for `Gen(i)` (otherwise `A`, `B`, ...).
    pub gen_names: Vec<String>,
}

impl<'a, N: TyNames> Printer<'a, N> {
    pub fn new(names: &'a N) -> Self {
        Printer { names, vars: FxHashMap::default(), gen_names: vec![] }
    }

    pub fn print(&mut self, t: &Ty) -> String {
        let mut s = String::new();
        self.go(t, &mut s);
        s
    }

    fn var(&mut self, v: TyVid) -> String {
        let n = self.vars.len();
        self.vars
            .entry(v)
            .or_insert_with(|| if n < 26 { ((b'A' + n as u8) as char).to_string() } else { format!("T{n}") })
            .clone()
    }

    fn go(&mut self, t: &Ty, s: &mut String) {
        match &**t {
            TyKind::Var(v) => {
                let n = self.var(*v);
                s.push_str(&n);
            }
            TyKind::Con(d, args) => {
                s.push_str(&self.names.def_name(*d));
                if !args.is_empty() {
                    s.push('<');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        self.go(a, s);
                    }
                    s.push('>');
                }
            }
            TyKind::Dyn(d, args) => {
                s.push_str("dyn ");
                self.go(&mk(TyKind::Con(*d, args.clone())), s);
            }
            TyKind::Fn(f) => {
                s.push_str(if f.once { "once fn(" } else { "fn(" });
                for (i, a) in f.params.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    self.go(a, s);
                }
                s.push_str(") -> ");
                self.go(&f.ret, s);
                let (labels, tail) = row_fields(&f.eff);
                let labels: Vec<_> = labels
                    .iter()
                    .map(|(l, _)| self.names.sym(*l).to_string())
                    .filter(|l| !l.starts_with('%'))
                    .collect();
                if !labels.is_empty() || matches!(&*tail, TyKind::RowEmpty) && false {
                    let _ = write!(s, " !{{{}}}", labels.join(", "));
                }
            }
            TyKind::Record(r) => {
                let (fields, tail) = row_fields(r);
                s.push('{');
                for (i, (l, ft)) in fields.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    let name = self.names.sym(*l);
                    if name.contains('/') {
                        let _ = write!(s, "[:{name}]: ");
                    } else {
                        let _ = write!(s, "{name}: ");
                    }
                    self.go(ft, s);
                }
                match &*tail {
                    TyKind::RowEmpty => {}
                    _ => {
                        if !fields.is_empty() {
                            s.push_str(", ");
                        }
                        s.push_str("..");
                        self.go(&tail, s);
                    }
                }
                s.push('}');
            }
            TyKind::Proj(b, n) => {
                self.go(b, s);
                s.push('.');
                s.push_str(self.names.sym(*n));
            }
            TyKind::Tuple(ts) if ts.is_empty() => s.push_str("Unit"),
            TyKind::Tuple(ts) => {
                s.push('(');
                for (i, a) in ts.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    self.go(a, s);
                }
                if ts.len() == 1 {
                    s.push(',');
                }
                s.push(')');
            }
            TyKind::RowEmpty => s.push_str("{}"),
            TyKind::RowExt(..) => self.go(&mk(TyKind::Record(t.clone())), s),
            TyKind::Ref(m, a) => {
                s.push_str(if *m { "&mut " } else { "&" });
                self.go(a, s);
            }
            TyKind::Skolem(k) => s.push_str(&self.names.skolem_name(*k)),
            TyKind::Gen(i) if (*i as usize) < self.gen_names.len() => {
                s.push_str(&self.gen_names[*i as usize])
            }
            TyKind::Gen(i) => {
                let n = *i as usize;
                if n < 26 {
                    s.push((b'A' + n as u8) as char);
                } else {
                    let _ = write!(s, "T{n}");
                }
            }
            TyKind::Forall(n, b) => {
                let _ = write!(s, "for<{n}> ");
                self.go(b, s);
            }
            TyKind::Bound(i) => {
                let _ = write!(s, "S{i}");
            }
            TyKind::Error => s.push_str("{error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use la_arena::Idx;

    fn con(i: u32) -> Ty {
        mk(TyKind::Con(Idx::from_raw(i.into()), vec![]))
    }

    #[test]
    fn unify_function_types() {
        let mut cx = InferCtxt::new(None);
        let (a, b) = (cx.fresh(), cx.fresh());
        let f =
            mk(TyKind::Fn(FnTy { params: vec![a.clone()], ret: b.clone(), eff: row_empty(), once: false }));
        let g = mk(TyKind::Fn(FnTy { params: vec![con(1)], ret: con(2), eff: row_empty(), once: false }));
        cx.unify(&f, &g).unwrap();
        assert_eq!(cx.resolve(&f), g);
    }

    #[test]
    fn occurs_check() {
        let mut cx = InferCtxt::new(None);
        let a = cx.fresh();
        let f = mk(TyKind::Tuple(vec![a.clone()]));
        assert!(matches!(cx.unify(&a, &f), Err(UnifyError::Occurs(..))));
    }

    #[test]
    fn rows_in_different_order_unify() {
        let interner = lang_base::Interner::new();
        let (x, y) = (interner.intern("x"), interner.intern("y"));
        let mut cx = InferCtxt::new(None);
        let r1 = record([(x, con(1)), (y, con(2))], row_empty());
        let r2 = record([(y, con(2)), (x, con(1))], row_empty());
        cx.unify(&r1, &r2).unwrap();
    }

    #[test]
    fn open_row_absorbs_extra_fields() {
        let interner = lang_base::Interner::new();
        let (x, y) = (interner.intern("x"), interner.intern("y"));
        let mut cx = InferCtxt::new(None);
        let tail = cx.fresh();
        let open = record([(x, con(1))], tail.clone());
        let closed = record([(x, con(1)), (y, con(2))], row_empty());
        cx.unify(&open, &closed).unwrap();
        assert_eq!(cx.resolve(&tail), row([(y, con(2))], row_empty()));
    }

    #[test]
    fn closed_row_rejects_extra_field() {
        let interner = lang_base::Interner::new();
        let (x, y) = (interner.intern("x"), interner.intern("y"));
        let mut cx = InferCtxt::new(None);
        let a = record([(x, con(1))], row_empty());
        let b = record([(x, con(1)), (y, con(2))], row_empty());
        assert!(matches!(cx.unify(&a, &b), Err(UnifyError::MissingField { .. })));
    }

    #[test]
    fn region_skolem_cannot_escape() {
        let interner = lang_base::Interner::new();
        let mut cx = InferCtxt::new(None);
        let outer = cx.fresh();
        cx.level += 1;
        let s = cx.fresh_skolem(interner.intern("S"), true);
        let inner = cx.fresh();
        cx.unify(&inner, &s).unwrap();
        cx.level -= 1;
        assert!(matches!(cx.unify(&outer, &inner), Err(UnifyError::Escape { .. })));
    }
}
