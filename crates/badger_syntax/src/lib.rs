//! Surface syntax: the lexer, the grammar, lowering to the AST, the AST
//! itself, and the formatter.
//!
//! [`token::lex`] (logos) turns source text into a flat buffer of tokens
//! and spans, and [`token::layout`] turns the newlines that end statements
//! into separators. The chumsky grammar ([`grammar`]) parses the token
//! slice into an owned syntax tree ([`cst`]), recovering from syntax errors
//! so that one pass reports all of them; [`lower`] turns that tree into the
//! arena-allocated AST ([`ast::Ast`]), whose nodes are addressed by typed
//! indices and carry spans.
#![allow(clippy::type_complexity)]

pub mod ast;
pub mod cst;
pub mod format;
pub mod grammar;
pub mod lower;
pub mod parser;
pub mod spacing;
pub mod token;

pub use parser::{ParseError, parse, parse_recovering};

/// Words that cannot name anything ([`token::is_reserved`]). Others,
/// such as `system`, `pipeline`, `map`, `set`, and `ref`, are keywords only
/// in their own constructs and may be names.
pub const RESERVED: &[&str] = &[
    "import", "as", "pub", "type", "struct", "enum", "protocol", "impl", "for", "where", "fn", "once", "let",
    "var", "const", "if", "else", "match", "while", "loop", "in", "break", "continue", "return", "use",
    "unsafe", "extern", "dyn", "true", "false", "mut",
];

#[cfg(test)]
mod tests {
    /// [`RESERVED`](super::RESERVED) lists exactly the lexer's reserved words.
    #[test]
    fn reserved_matches_lexer() {
        assert!(super::RESERVED.iter().all(|w| crate::token::is_reserved(w)));
        for w in ["system", "pipeline", "map", "set", "ref", "requires", "ensures", "derive", "repr", "self"]
        {
            assert!(!crate::token::is_reserved(w), "{w}");
        }
        assert_eq!(super::RESERVED.len(), 31);
    }
}
