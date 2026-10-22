//! High-level IR: the AST after module/name resolution and desugaring.
//!
//! - modules are collected, imports resolved (with the prelude implicitly
//!   imported), and every name resolved to a local, a definition, an enum
//!   constructor, or a type namespace
//! - surface sugar is removed: pipes become calls, `use` binds the rest of the
//!   block as a callback, dotted record updates and field assignments become
//!   nested row-preserving updates, and system configurations become deferred
//!   lambdas whose `ref(#key)` holes read resolved outputs
//! - lambdas record their captured locals
//! - syntactic system checks (missing references, dependency cycles) run here
//!
//! Like the AST, HIR lives in `la_arena` arenas addressed by typed `Idx`es;
//! per-node analysis results are stored in side tables by later phases.
#![allow(clippy::type_complexity)]

pub mod hir;
pub mod lower;

pub use hir::*;
pub use lower::{ParsedModule, lower_program};
