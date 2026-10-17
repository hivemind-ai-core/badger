//! Static contract checking: what the compiler can decide about `requires`
//! and `ensures` clauses before the program runs.
//!
//! A small abstract interpreter follows each function body, knowing values
//! as integer ranges, booleans, texts, and list lengths. It learns from
//! constants, immutable `let`s, the conditions of enclosing `if`s (and of
//! guard clauses that return early), and the `ensures` clauses of the
//! functions it calls. With that it
//!
//! - rejects a call whose precondition cannot hold there (E0145), and a
//!   return whose value breaks the function's postcondition (E0146);
//! - proves clauses, so their runtime checks are compiled out: a
//!   postcondition proven at every return, and a precondition proven at
//!   every call of a function that is only ever called directly.
//!
//! Anything it cannot decide stays a runtime check, as before.

use lang_base::{Diagnostic, Interner, Span};
use lang_hir::{
    BinOp, DefId, DefKind, ExprId, ExprKind, FnDef, Hir, Lit, LocalId, Owner, PatKind, Stmt, UnOp,
};
use std::collections::{HashMap, HashSet};

/// What the analysis knows about a value.
#[derive(Clone, Debug, PartialEq)]
enum V {
    /// An integer in `lo..=hi`.
    Int(i128, i128),
    Bool(Option<bool>),
    Str(Option<String>),
    /// A list with between `lo` and `hi` items.
    List(i128, i128),
    Unknown,
}

const MIN: i128 = i64::MIN as i128;
const MAX: i128 = i64::MAX as i128;

fn int(lo: i128, hi: i128) -> V {
    // a value outside `Int` would have trapped (overflow is a defect)
    V::Int(lo.max(MIN), hi.min(MAX))
}

fn empty(v: &V) -> bool {
    matches!(v, V::Int(lo, hi) | V::List(lo, hi) if lo > hi)
}

fn join(a: &V, b: &V) -> V {
    match (a, b) {
        _ if empty(a) => b.clone(),
        _ if empty(b) => a.clone(),
        (V::Int(a1, a2), V::Int(b1, b2)) => V::Int(*a1.min(b1), *a2.max(b2)),
        (V::List(a1, a2), V::List(b1, b2)) => V::List(*a1.min(b1), *a2.max(b2)),
        (V::Bool(x), V::Bool(y)) if x == y => V::Bool(*x),
        (V::Bool(_), V::Bool(_)) => V::Bool(None),
        (V::Str(x), V::Str(y)) if x == y => V::Str(x.clone()),
        (V::Str(_), V::Str(_)) => V::Str(None),
        _ => V::Unknown,
    }
}

/// Facts about immutable locals on one path.
#[derive(Clone, Default)]
struct Env {
    vals: HashMap<LocalId, V>,
    /// The path cannot be taken (its conditions contradict).
    dead: bool,
}

impl Env {
    fn get(&self, l: LocalId) -> V {
        self.vals.get(&l).cloned().unwrap_or(V::Unknown)
    }

    /// Narrow `l` to `v` (intersecting ranges); a contradiction kills the path.
    fn narrow(&mut self, l: LocalId, v: V) {
        let cur = self.get(l);
        let next = match (&cur, &v) {
            (V::Int(a1, a2), V::Int(b1, b2)) => V::Int(*a1.max(b1), *a2.min(b2)),
            (V::Unknown, V::Int(b1, b2)) => V::Int(*b1, *b2),
            (V::List(a1, a2), V::List(b1, b2)) => V::List(*a1.max(b1), *a2.min(b2)),
            (V::Unknown, _) => v.clone(),
            _ => cur.clone(),
        };
        if empty(&next) {
            self.dead = true;
        }
        self.vals.insert(l, next);
    }
}

/// A clause the analysis decided: proven, or refuted at a span.
struct Analysis<'a> {
    hir: &'a Hir,
    interner: &'a Interner,
    targets: &'a HashMap<ExprId, DefId>,
    /// Locals assigned anywhere (their values are not tracked).
    assigned: HashSet<LocalId>,
    diags: Vec<Diagnostic>,
    /// Per function with preconditions: whether every call site proved
    /// each clause, and whether it is ever used other than by a direct call.
    pre_ok: HashMap<DefId, Vec<bool>>,
    pre_unproven: HashMap<DefId, Vec<usize>>,
    calls: HashMap<DefId, usize>,
    escapes: HashSet<DefId>,
    /// The function whose body is being walked, and its return points.
    current: Option<DefId>,
    returns: Vec<(ExprId, Env)>,
}

fn is_std(hir: &Hir, span: Span) -> bool {
    hir.modules.iter().any(|(_, m)| m.is_std && m.file == span.file)
}

impl<'a> Analysis<'a> {
    fn fn_def(&self, d: DefId) -> Option<&'a FnDef> {
        match &self.hir.defs[d].kind {
            DefKind::Fn(f) => Some(f),
            _ => None,
        }
    }

    fn name(&self, d: DefId) -> String {
        self.interner.resolve(self.hir.defs[d].name).to_string()
    }

    // ------------------------------------------------------------ evaluation

    fn eval(&self, e: ExprId, env: &Env) -> V {
        let ex = &self.hir.exprs[e];
        match &ex.kind {
            ExprKind::Lit(Lit::Int { value, .. }) => {
                let v = *value as i128;
                int(v, v)
            }
            ExprKind::Lit(Lit::Bool(b)) => V::Bool(Some(*b)),
            ExprKind::Lit(Lit::Str(s)) => V::Str(Some(s.clone())),
            ExprKind::Local(l) => env.get(*l),
            ExprKind::List(items) => {
                let n = items.len() as i128;
                V::List(n, n)
            }
            ExprKind::Unary { op: UnOp::Neg, operand } => match self.eval(*operand, env) {
                V::Int(lo, hi) => int(-hi, -lo),
                _ => V::Unknown,
            },
            ExprKind::Unary { op: UnOp::Not, operand } => match self.eval(*operand, env) {
                V::Bool(b) => V::Bool(b.map(|b| !b)),
                _ => V::Unknown,
            },
            ExprKind::Binary { op, lhs, rhs } => self.binary(*op, *lhs, *rhs, env),
            ExprKind::MethodCall { recv, method, args, .. } if args.is_empty() => {
                let m = self.interner.resolve(*method);
                match (m, self.eval(*recv, env)) {
                    ("len", V::List(lo, hi)) => V::Int(lo, hi),
                    ("len", V::Str(Some(s))) => {
                        let n = s.chars().count() as i128;
                        V::Int(n, n)
                    }
                    ("len", _) => int(0, MAX),
                    ("is_empty", V::List(lo, hi)) => V::Bool(if hi == 0 {
                        Some(true)
                    } else if lo > 0 {
                        Some(false)
                    } else {
                        None
                    }),
                    ("is_empty", V::Str(Some(s))) => V::Bool(Some(s.is_empty())),
                    _ => self.call_result(e, env),
                }
            }
            ExprKind::Call { .. } | ExprKind::MethodCall { .. } => self.call_result(e, env),
            ExprKind::If { cond, then, els } => {
                let mut t = env.clone();
                self.refine(*cond, true, &mut t);
                let mut f = env.clone();
                self.refine(*cond, false, &mut f);
                let a = if t.dead { None } else { Some(self.eval(*then, &t)) };
                let b = match els {
                    Some(x) if !f.dead => Some(self.eval(*x, &f)),
                    Some(_) => None,
                    None => Some(V::Unknown),
                };
                match (a, b) {
                    (Some(a), Some(b)) => join(&a, &b),
                    (Some(a), None) => a,
                    (None, Some(b)) => b,
                    (None, None) => V::Unknown,
                }
            }
            ExprKind::Block { stmts, tail } => {
                let mut env = env.clone();
                for s in stmts {
                    self.bind(s, &mut env);
                    if let Stmt::Expr { expr, .. } = s
                        && let ExprKind::If { cond, then, els: None } = &self.hir.exprs[*expr].kind
                        && self.diverges(*then)
                    {
                        self.refine(*cond, false, &mut env);
                    }
                }
                if env.dead {
                    return V::Int(1, 0);
                }
                tail.map_or(V::Unknown, |t| self.eval(t, &env))
            }
            _ => V::Unknown,
        }
    }

    /// What a call's `ensures` clauses say about its result.
    fn call_result(&self, call: ExprId, env: &Env) -> V {
        let Some(d) = self.callee(call) else { return V::Unknown };
        let Some(f) = self.fn_def(d) else { return V::Unknown };
        let (Some(r), false) = (f.result, f.ensures.is_empty()) else { return V::Unknown };
        // assume the callee's postconditions (checked in it, or proven)
        let mut callee_env = self.call_env(call, f, env);
        for pc in &f.ensures {
            self.refine(pc.cond, true, &mut callee_env);
        }
        callee_env.get(r)
    }

    fn binary(&self, op: BinOp, lhs: ExprId, rhs: ExprId, env: &Env) -> V {
        if matches!(op, BinOp::And | BinOp::Or) {
            let a = self.eval(lhs, env);
            let mut env2 = env.clone();
            self.refine(lhs, op == BinOp::And, &mut env2);
            let b = if env2.dead { V::Unknown } else { self.eval(rhs, &env2) };
            return match (op, a, b) {
                (BinOp::And, V::Bool(Some(false)), _) | (BinOp::And, _, V::Bool(Some(false))) => {
                    V::Bool(Some(false))
                }
                (BinOp::And, V::Bool(Some(true)), V::Bool(Some(true))) => V::Bool(Some(true)),
                (BinOp::Or, V::Bool(Some(true)), _) | (BinOp::Or, _, V::Bool(Some(true))) => {
                    V::Bool(Some(true))
                }
                (BinOp::Or, V::Bool(Some(false)), V::Bool(Some(false))) => V::Bool(Some(false)),
                _ => V::Bool(None),
            };
        }
        let (a, b) = (self.eval(lhs, env), self.eval(rhs, env));
        // both operands have one type: beside an integer, an unknown is
        // some integer
        let (a, b) = match (a, b) {
            (V::Unknown, b @ V::Int(..)) => (V::Int(MIN, MAX), b),
            (a @ V::Int(..), V::Unknown) => (a, V::Int(MIN, MAX)),
            other => other,
        };
        match (a, b) {
            (V::Int(a1, a2), V::Int(b1, b2)) => match op {
                BinOp::Add => int(a1 + b1, a2 + b2),
                BinOp::Sub => int(a1 - b2, a2 - b1),
                BinOp::Mul => {
                    let ps = [a1 * b1, a1 * b2, a2 * b1, a2 * b2];
                    int(*ps.iter().min().unwrap(), *ps.iter().max().unwrap())
                }
                // by a positive constant: the remainder takes the
                // dividend's sign; the quotient truncates toward zero
                BinOp::Rem if b1 == b2 && b1 > 0 => {
                    if a1 >= 0 {
                        V::Int(0, (b1 - 1).min(a2))
                    } else if a2 <= 0 {
                        V::Int((1 - b1).max(a1), 0)
                    } else {
                        V::Int(1 - b1, b1 - 1)
                    }
                }
                BinOp::Div if b1 == b2 && b1 > 0 => V::Int(a1 / b1, a2 / b1),
                BinOp::Lt => cmp(a2 < b1, a1 >= b2),
                BinOp::Le => cmp(a2 <= b1, a1 > b2),
                BinOp::Gt => cmp(a1 > b2, a2 <= b1),
                BinOp::Ge => cmp(a1 >= b2, a2 < b1),
                BinOp::Eq => cmp(a1 == a2 && b1 == b2 && a1 == b1, a2 < b1 || b2 < a1),
                BinOp::Ne => cmp(a2 < b1 || b2 < a1, a1 == a2 && b1 == b2 && a1 == b1),
                _ => V::Unknown,
            },
            (V::Str(Some(x)), V::Str(Some(y))) => match op {
                BinOp::Eq => V::Bool(Some(x == y)),
                BinOp::Ne => V::Bool(Some(x != y)),
                _ => V::Unknown,
            },
            (V::Bool(Some(x)), V::Bool(Some(y))) => match op {
                BinOp::Eq => V::Bool(Some(x == y)),
                BinOp::Ne => V::Bool(Some(x != y)),
                _ => V::Unknown,
            },
            _ => match op {
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => V::Bool(None),
                _ => V::Unknown,
            },
        }
    }

    // ------------------------------------------------------------ refinement

    /// Learn from `cond` being `truth` on this path.
    fn refine(&self, cond: ExprId, truth: bool, env: &mut Env) {
        match &self.hir.exprs[cond].kind {
            ExprKind::Unary { op: UnOp::Not, operand } => self.refine(*operand, !truth, env),
            ExprKind::Binary { op: BinOp::And, lhs, rhs } if truth => {
                self.refine(*lhs, true, env);
                self.refine(*rhs, true, env);
            }
            ExprKind::Binary { op: BinOp::Or, lhs, rhs } if !truth => {
                self.refine(*lhs, false, env);
                self.refine(*rhs, false, env);
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let op = if truth { Some(*op) } else { negate(*op) };
                let Some(op) = op else { return };
                // `local OP value`, `value OP local`, and `list.len() OP value`
                if let Some((l, list)) = self.tracked(*lhs) {
                    self.narrow_by(l, list, op, self.eval(*rhs, env), env);
                }
                if let Some((l, list)) = self.tracked(*rhs) {
                    self.narrow_by(l, list, flip(op), self.eval(*lhs, env), env);
                }
            }
            ExprKind::Lit(Lit::Bool(b)) if *b != truth => env.dead = true,
            _ => {}
        }
        if let V::Bool(Some(b)) = self.eval(cond, env)
            && b != truth
        {
            env.dead = true;
        }
    }

    /// The tracked local an operand reads: itself, or a list's length.
    fn tracked(&self, e: ExprId) -> Option<(LocalId, bool)> {
        match &self.hir.exprs[e].kind {
            ExprKind::Local(l) if !self.assigned.contains(l) => Some((*l, false)),
            ExprKind::MethodCall { recv, method, args, .. }
                if args.is_empty() && self.interner.resolve(*method) == "len" =>
            {
                match &self.hir.exprs[*recv].kind {
                    ExprKind::Local(l) if !self.assigned.contains(l) => Some((*l, true)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn narrow_by(&self, l: LocalId, list: bool, op: BinOp, other: V, env: &mut Env) {
        let V::Int(lo, hi) = other else { return };
        let range = match op {
            BinOp::Lt => (MIN, hi - 1),
            BinOp::Le => (MIN, hi),
            BinOp::Gt => (lo + 1, MAX),
            BinOp::Ge => (lo, MAX),
            BinOp::Eq => (lo, hi),
            BinOp::Ne if lo == hi => {
                // only an endpoint can be cut off
                match env.get(l) {
                    V::Int(a, b) if a == lo => (a + 1, b),
                    V::Int(a, b) if b == lo => (a, b - 1),
                    _ => return,
                }
            }
            _ => return,
        };
        if list {
            env.narrow(l, V::List(range.0.max(0), range.1));
        } else {
            env.narrow(l, V::Int(range.0, range.1));
        }
    }

    fn bind(&self, s: &Stmt, env: &mut Env) {
        if let Stmt::Let { pat, init, .. } = s
            && let PatKind::Bind(l) = &self.hir.pats[*pat].kind
            && !self.assigned.contains(l)
        {
            let v = self.eval(*init, env);
            env.vals.insert(*l, v);
        }
    }

    // ------------------------------------------------------------ calls

    fn callee(&self, call: ExprId) -> Option<DefId> {
        match &self.hir.exprs[call].kind {
            ExprKind::Call { callee, .. } => match &self.hir.exprs[*callee].kind {
                ExprKind::Def { def, .. } => Some(*def),
                _ => self.targets.get(callee).copied(),
            },
            ExprKind::MethodCall { .. } => self.targets.get(&call).copied(),
            _ => None,
        }
    }

    /// The callee's parameters bound to the call's arguments.
    fn call_env(&self, call: ExprId, f: &FnDef, env: &Env) -> Env {
        let args: Vec<ExprId> = match &self.hir.exprs[call].kind {
            ExprKind::Call { args, .. } => args.clone(),
            ExprKind::MethodCall { recv, args, .. } => {
                let mut v = vec![*recv];
                v.extend(args.iter().copied());
                v
            }
            _ => vec![],
        };
        let mut out = Env::default();
        if args.len() != f.params.len() {
            return out;
        }
        for (p, a) in f.params.iter().zip(args) {
            if let PatKind::Bind(l) = &self.hir.pats[p.pat].kind {
                out.vals.insert(*l, self.eval(a, env));
            }
        }
        out
    }

    fn check_call(&mut self, call: ExprId, env: &Env) {
        let Some(d) = self.callee(call) else { return };
        let Some(f) = self.fn_def(d) else { return };
        if f.requires.is_empty() {
            return;
        }
        let callee_env = self.call_env(call, f, env);
        let proven: Vec<bool> = f
            .requires
            .iter()
            .map(|pc| {
                let v = self.eval(pc.cond, &callee_env);
                if v == V::Bool(Some(false)) && !env.dead {
                    let span = self.hir.exprs[call].span;
                    self.diags.push(
                        Diagnostic::error(format!(
                            "this call breaks the precondition `{}` of `{}`",
                            pc.text,
                            self.name(d)
                        ))
                        .with_code("E0145")
                        .with_label(span, "the arguments here cannot satisfy it")
                        .with_label(pc.span, "the precondition"),
                    );
                }
                v == V::Bool(Some(true)) || env.dead
            })
            .collect();
        *self.calls.entry(d).or_insert(0) += 1;
        let misses = self.pre_unproven.entry(d).or_insert_with(|| vec![0; proven.len()]);
        for (m, p) in misses.iter_mut().zip(&proven) {
            if !p {
                *m += 1;
            }
        }
        let entry = self.pre_ok.entry(d).or_insert_with(|| vec![true; proven.len()]);
        for (e, p) in entry.iter_mut().zip(proven) {
            *e &= p;
        }
    }

    // ------------------------------------------------------------ walking

    /// Visit `e` on the path `env`, checking calls and recording returns.
    fn walk(&mut self, e: ExprId, env: &Env) {
        let kind = self.hir.exprs[e].kind.clone();
        match kind {
            ExprKind::Def { def, .. } => {
                // a function used as a value may be called anywhere
                self.escapes.insert(def);
            }
            ExprKind::AssocFn { .. } => {
                if let Some(d) = self.targets.get(&e) {
                    self.escapes.insert(*d);
                }
            }
            ExprKind::Try(x) => {
                self.walk(x, env);
                // an error leaves the function here, with an unknown value
                if self.current.is_some() {
                    self.returns.push((e, env.clone()));
                }
            }
            ExprKind::Call { callee, args } => {
                if !matches!(self.hir.exprs[callee].kind, ExprKind::Def { .. } | ExprKind::AssocFn { .. }) {
                    self.walk(callee, env);
                }
                for a in args {
                    self.walk(a, env);
                }
                self.check_call(e, env);
            }
            ExprKind::MethodCall { recv, args, .. } => {
                self.walk(recv, env);
                for a in args {
                    self.walk(a, env);
                }
                self.check_call(e, env);
            }
            ExprKind::If { cond, then, els } => {
                self.walk(cond, env);
                let mut t = env.clone();
                self.refine(cond, true, &mut t);
                self.walk(then, &t);
                if let Some(x) = els {
                    let mut f = env.clone();
                    self.refine(cond, false, &mut f);
                    self.walk(x, &f);
                }
            }
            ExprKind::Block { stmts, tail } => {
                let mut env = env.clone();
                for s in &stmts {
                    match s {
                        Stmt::Let { init, .. } => {
                            self.walk(*init, &env);
                            self.bind(s, &mut env);
                        }
                        Stmt::Assign { value, .. } => self.walk(*value, &env),
                        Stmt::Expr { expr, .. } => {
                            self.walk(*expr, &env);
                            // a guard clause: `if c { return }` leaves `!c`
                            if let ExprKind::If { cond, then, els: None } = &self.hir.exprs[*expr].kind
                                && self.diverges(*then)
                            {
                                self.refine(*cond, false, &mut env);
                            }
                        }
                    }
                }
                if let Some(t) = tail {
                    self.walk(t, &env);
                    if self.current_body_tail(e) {
                        self.returns.push((t, env.clone()));
                    }
                }
            }
            ExprKind::Return { value, target } => {
                if let Some(v) = value {
                    self.walk(v, env);
                    if matches!(target, Owner::Fn(d) if Some(d) == self.current) {
                        self.returns.push((v, env.clone()));
                    }
                }
            }
            ExprKind::Lambda(l) => {
                // a lambda's returns are its own
                let saved = self.current.take();
                let returns = std::mem::take(&mut self.returns);
                self.walk(l.body, env);
                self.returns = returns;
                self.current = saved;
            }
            other => {
                for c in children(&other) {
                    self.walk(c, env);
                }
            }
        }
    }

    /// Whether `e` is the body (the outermost block) of the current function.
    fn current_body_tail(&self, e: ExprId) -> bool {
        self.current.and_then(|d| self.fn_def(d)).and_then(|f| f.body) == Some(e)
    }

    /// Whether a branch always leaves (returns, breaks, continues, or panics).
    fn diverges(&self, e: ExprId) -> bool {
        match &self.hir.exprs[e].kind {
            ExprKind::Return { .. } | ExprKind::Break { .. } | ExprKind::Continue { .. } => true,
            ExprKind::Block { stmts, tail } => {
                tail.is_some_and(|t| self.diverges(t))
                    || stmts
                        .last()
                        .is_some_and(|s| matches!(s, Stmt::Expr { expr, .. } if self.diverges(*expr)))
            }
            ExprKind::Call { callee, .. } => matches!(
                &self.hir.exprs[*callee].kind,
                ExprKind::Def { def, .. } if self.interner.resolve(self.hir.defs[*def].name) == "panic"
            ),
            _ => false,
        }
    }

    /// Check a function's postconditions at each of its return points.
    fn check_returns(&mut self, d: DefId, f: &FnDef, elided: &mut HashSet<(DefId, usize)>) {
        let Some(r) = f.result else { return };
        let returns = std::mem::take(&mut self.returns);
        for (k, pc) in f.ensures.iter().enumerate() {
            let mut all = !returns.is_empty();
            for (value, env) in &returns {
                if env.dead {
                    continue;
                }
                let mut env = env.clone();
                let v = self.eval(*value, &env);
                env.vals.insert(r, v);
                match self.eval(pc.cond, &env) {
                    V::Bool(Some(true)) => {}
                    V::Bool(Some(false)) => {
                        all = false;
                        self.diags.push(
                            Diagnostic::error(format!(
                                "`{}` returns a value that breaks its postcondition",
                                self.name(d)
                            ))
                            .with_code("E0146")
                            .with_label(self.hir.exprs[*value].span, "this value cannot satisfy it")
                            .with_label(pc.span, "the postcondition"),
                        );
                    }
                    _ => all = false,
                }
            }
            if all {
                elided.insert((d, k));
            }
        }
    }
}

fn cmp(always: bool, never: bool) -> V {
    V::Bool(if always {
        Some(true)
    } else if never {
        Some(false)
    } else {
        None
    })
}

fn negate(op: BinOp) -> Option<BinOp> {
    Some(match op {
        BinOp::Lt => BinOp::Ge,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        BinOp::Ge => BinOp::Lt,
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        _ => return None,
    })
}

fn flip(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Gt,
        BinOp::Le => BinOp::Ge,
        BinOp::Gt => BinOp::Lt,
        BinOp::Ge => BinOp::Le,
        o => o,
    }
}

/// The direct subexpressions of an expression (those `walk` does not
/// handle itself).
fn children(k: &ExprKind) -> Vec<ExprId> {
    let mut v = vec![];
    match k {
        ExprKind::Field { base, .. } => v.push(*base),
        ExprKind::Index { base, index } => v.extend([*base, *index]),
        ExprKind::Unsafe(x) | ExprKind::Unary { operand: x, .. } => v.push(*x),
        ExprKind::Binary { lhs, rhs, .. } => v.extend([*lhs, *rhs]),
        ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => v.extend(xs.iter().copied()),
        ExprKind::Record { spread, fields } => {
            v.extend(spread.iter().copied());
            v.extend(fields.iter().map(|f| f.2));
        }
        ExprKind::Map(kvs) => {
            for (a, b) in kvs {
                v.extend([*a, *b]);
            }
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
        ExprKind::Loop { body } => v.push(*body),
        ExprKind::Break { value, .. } => v.extend(value.iter().copied()),
        ExprKind::Interp(parts) => {
            for p in parts {
                if let lang_hir::InterpPart::Expr(e) = p {
                    v.push(*e);
                }
            }
        }
        _ => {}
    }
    v
}

/// Whether any user function states a contract (the analysis needs the
/// type checker's call targets).
pub fn wanted(hir: &Hir) -> bool {
    hir.defs.iter().any(|(_, d)| {
        matches!(&d.kind, DefKind::Fn(f) if !f.requires.is_empty() || !f.ensures.is_empty())
            && !is_std(hir, d.span)
    })
}

/// One clause and what the analysis decided about it.
#[derive(Debug, Clone)]
pub struct Clause {
    pub function: String,
    /// `requires` or `ensures`.
    pub kind: &'static str,
    pub text: String,
    pub file: String,
    pub line: usize,
    /// `proven` (its runtime check is compiled out), `refuted` (an error
    /// was reported), or `runtime` (checked when the program runs).
    pub status: &'static str,
    /// Why, in a phrase (`proven at all 3 calls`, `the function is used as
    /// a value`, ...).
    pub reason: String,
    /// Direct calls of the function (for `requires`).
    pub calls: usize,
    /// Where the clause is (the caller fills in `file` and `line`).
    pub span: Span,
}

impl Clause {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "function": self.function,
            "kind": self.kind,
            "text": self.text,
            "file": self.file,
            "line": self.line,
            "status": self.status,
            "reason": self.reason,
            "calls": self.calls,
        })
    }
}

thread_local! {
    static LAST: std::cell::RefCell<Vec<Clause>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The clauses of the program checked last on this thread.
pub fn last_report() -> Vec<Clause> {
    LAST.with(|l| l.borrow().clone())
}

/// Check the program's contracts; the diagnostics, and the checks proven
/// never to fail (to compile out).
pub fn check(
    hir: &Hir,
    types: &lang_typeck::TypeIndex,
    interner: &Interner,
) -> (Vec<Diagnostic>, lang_mir::lower::Elided) {
    let (diags, elided, report) = analyze(hir, types, interner);
    LAST.with(|l| *l.borrow_mut() = report);
    (diags, elided)
}

fn analyze(
    hir: &Hir,
    types: &lang_typeck::TypeIndex,
    interner: &Interner,
) -> (Vec<Diagnostic>, lang_mir::lower::Elided, Vec<Clause>) {
    let targets: HashMap<ExprId, DefId> = types.targets.iter().map(|(k, v)| (*k, *v)).collect();
    let mut assigned = HashSet::new();
    for (_, ex) in hir.exprs.iter() {
        if let ExprKind::Block { stmts, .. } = &ex.kind {
            for s in stmts {
                if let Stmt::Assign { local, .. } = s {
                    assigned.insert(*local);
                }
            }
        }
    }
    let mut a = Analysis {
        hir,
        interner,
        targets: &targets,
        assigned,
        diags: vec![],
        pre_ok: HashMap::new(),
        pre_unproven: HashMap::new(),
        calls: HashMap::new(),
        escapes: HashSet::new(),
        current: None,
        returns: vec![],
    };
    let mut elided = lang_mir::lower::Elided::default();
    let fns: Vec<(DefId, &FnDef)> = hir
        .defs
        .iter()
        .filter_map(|(d, def)| match &def.kind {
            DefKind::Fn(f) if f.body.is_some() => Some((d, f)),
            _ => None,
        })
        .collect();
    for (d, f) in &fns {
        let body = f.body.unwrap();
        a.current = Some(*d);
        a.returns.clear();
        // inside the body its preconditions hold (checked on entry, or
        // proven at every call)
        let mut entry = Env::default();
        for pc in &f.requires {
            a.refine(pc.cond, true, &mut entry);
        }
        a.walk(body, &entry);
        if !f.ensures.is_empty() && !is_std(hir, hir.defs[*d].span) {
            a.check_returns(*d, f, &mut elided.posts);
        }
    }
    // preconditions proven at every call of a function only called directly
    for (d, f) in &fns {
        if f.requires.is_empty() || a.escapes.contains(d) || is_std(hir, hir.defs[*d].span) {
            continue;
        }
        // a function never called keeps its checks (an entry point, a test)
        if let Some(ok) = a.pre_ok.get(d) {
            for (pc, proven) in f.requires.iter().zip(ok) {
                if *proven {
                    elided.checks.insert(pc.check);
                }
            }
        }
    }
    // the report
    let refuted: HashSet<Span> = a.diags.iter().flat_map(|d| d.labels.iter().map(|l| l.span)).collect();
    let mut report = vec![];
    for (d, f) in &fns {
        if is_std(hir, hir.defs[*d].span) {
            continue;
        }
        let name = a.name(*d);
        let calls = *a.calls.get(d).unwrap_or(&0);
        for (k, pc) in f.requires.iter().enumerate() {
            let (status, reason) = if refuted.contains(&pc.span) {
                ("refuted", "a call cannot satisfy it".to_string())
            } else if elided.checks.contains(&pc.check) {
                ("proven", format!("holds at all {calls} call(s)"))
            } else if a.escapes.contains(d) {
                ("runtime", "the function is used as a value, so not every call is known".into())
            } else if calls == 0 {
                ("runtime", "no direct calls (an entry point, or called from tests)".into())
            } else {
                let unproven = a.pre_unproven.get(d).and_then(|v| v.get(k)).copied().unwrap_or(0);
                ("runtime", format!("not proven at {unproven} of {calls} call(s)"))
            };
            report.push(clause(hir, &name, "requires", &pc.text, pc.span, status, reason, calls));
        }
        for (k, pc) in f.ensures.iter().enumerate() {
            let text = clause_text(f, k);
            let (status, reason) = if refuted.contains(&pc.span) {
                ("refuted", "a return value cannot satisfy it".to_string())
            } else if elided.posts.contains(&(*d, k)) {
                ("proven", "holds at every return".to_string())
            } else {
                ("runtime", "not proven at every return".to_string())
            };
            report.push(clause(hir, &name, "ensures", &text, pc.span, status, reason, calls));
        }
    }
    (a.diags, elided, report)
}

/// The source text of postcondition `k` (from the function's clause list).
fn clause_text(f: &FnDef, k: usize) -> String {
    f.contracts
        .iter()
        .filter_map(|c| c.strip_prefix("ensures "))
        .nth(k)
        .map(|c| c.split(", \"").next().unwrap_or(c).trim().to_string())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn clause(
    hir: &Hir,
    function: &str,
    kind: &'static str,
    text: &str,
    span: Span,
    status: &'static str,
    reason: String,
    calls: usize,
) -> Clause {
    let _ = hir;
    Clause {
        function: function.to_string(),
        kind,
        text: text.to_string(),
        file: String::new(),
        line: 0,
        status,
        reason,
        calls,
        span,
    }
}
