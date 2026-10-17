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

/// Words that cannot name anything (the grammar's `reserved` rule). Others,
/// such as `system`, `pipeline`, `map`, `set`, and `ref`, are keywords only
/// in their own constructs and may be names.
pub const RESERVED: &[&str] = &[
    "import", "as", "pub", "type", "struct", "enum", "protocol", "impl", "for", "where", "fn", "once", "let",
    "var", "const", "if", "else", "match", "while", "loop", "in", "break", "continue", "return", "use",
    "unsafe", "extern", "dyn", "true", "false", "mut",
];

#[cfg(test)]
mod tests {
    /// [`RESERVED`](super::RESERVED) lists exactly the grammar's `reserved` words.
    #[test]
    fn reserved_matches_grammar() {
        let g = include_str!("grammar.pest");
        let start = g.find("reserved   = @{").expect("the reserved rule");
        let rule = &g[start..start + g[start..].find("~ !ident_char").expect("its end")];
        let mut words: Vec<&str> = rule.split('"').skip(1).step_by(2).collect();
        let mut listed = super::RESERVED.to_vec();
        words.sort();
        listed.sort();
        assert_eq!(words, listed);
    }
}
