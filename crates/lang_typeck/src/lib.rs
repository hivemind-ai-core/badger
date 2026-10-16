//! Type inference and type-directed static analysis over HIR.
//!
//! - [`ty`]: types, rows, and unification (levels, occurs check, skolem escape)
//! - [`env`]: declared types, protocols, impls (including derived impls)
//! - [`infer`]: bidirectional expression/pattern inference
//! - [`solve`]: deferred constraints (protocols, methods, operators, row
//!   updates, iteration, `?` conversions)
//! - [`program`]: binding groups, generalization, and per-instance re-checking
//! - [`exhaust`], [`ownership`]: match exhaustiveness and affine use checks
#![allow(clippy::type_complexity)]

pub mod env;
pub mod exhaust;
pub mod infer;
pub mod ownership;
pub mod program;
pub mod regions;
pub mod solve;
pub mod ty;

pub use infer::{Coercion, IndexKind, IterKind, Res, Target};
pub use program::{
    Checked, Global, Instance, Member, Scheme, TypeIndex, check_instance, check_program, ground,
    ground_resolved, set_record_types, set_repl_session, show_ty, subst_instance,
};
pub use ty::{FnTy, InferCtxt, Ty, TyKind, TyVid, UnifyError};
