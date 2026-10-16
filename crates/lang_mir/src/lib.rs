//! Mid-level IR: typed, explicit control-flow graphs between HIR and
//! Cranelift, produced by monomorphization.
//!
//! - [`layout`]: concrete layouts and runtime descriptors for ground types
//! - [`lower`]: instance worklist and typed-HIR → MIR lowering
//! - [`mir`]: the IR itself (locals, blocks, statements, terminators)
#![allow(clippy::type_complexity, clippy::wrong_self_convention)]

pub mod inline;
pub mod layout;
pub mod lower;
pub mod mir;
pub mod roots;
pub mod serial;
pub mod sra;
pub mod verify;

pub use lower::lower_program;
pub use mir::*;
