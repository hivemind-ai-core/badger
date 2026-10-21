//! Foundation types shared by every phase of the compiler:
//! source files and spans, interned symbols, and diagnostics.
#![allow(clippy::type_complexity)]

pub mod catalog;
pub mod diagnostics;
pub mod intern;
pub mod source;
pub mod suggest;

pub use diagnostics::{Diagnostic, Diagnostics, Label, Severity};
pub use intern::{Interner, Symbol};
pub use source::{FileId, SourceFile, SourceMap, Span};
