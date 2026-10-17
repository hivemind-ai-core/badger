//! Surface syntax: layout pass, pest grammar, parse-tree → AST lowering, and
//! the AST itself.
//!
//! Source text first goes through [`layout::layout`], which replaces
//! statement-terminating newlines with an explicit separator (preserving byte
//! offsets). The pest parse tree (`Pairs`) is short-lived; it is immediately
//! lowered into an arena-allocated AST (`ast::Ast`) whose nodes are addressed by
//! typed indices and carry `Span`s. Operator precedence is handled with pest's
//! `PrattParser` during lowering rather than encoded in the grammar.
#![allow(clippy::type_complexity)]

pub mod ast;
pub mod format;
pub mod layout;
pub mod parser;
pub mod spacing;

pub use parser::{ParseError, parse};
