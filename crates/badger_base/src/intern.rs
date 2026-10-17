use lasso::{Spur, ThreadedRodeo};

/// Interned string (identifiers, keywords, string literals). Compares in O(1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(Spur);

impl Symbol {
    /// A placeholder symbol (the empty string's key in every interner).
    pub fn dummy() -> Symbol {
        Symbol(Spur::default())
    }
}

/// String interner. Interning takes `&self` so analysis phases holding a
/// shared reference can still create synthetic names.
#[derive(Debug)]
pub struct Interner {
    rodeo: ThreadedRodeo,
}

impl Default for Interner {
    fn default() -> Self {
        let rodeo = ThreadedRodeo::default();
        rodeo.get_or_intern("");
        Interner { rodeo }
    }
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&self, s: &str) -> Symbol {
        Symbol(self.rodeo.get_or_intern(s))
    }

    /// Look up an already-interned string, interning it if needed.
    pub fn get(&self, s: &str) -> Option<Symbol> {
        Some(self.intern(s))
    }

    pub fn resolve(&self, sym: Symbol) -> &str {
        self.rodeo.resolve(&sym.0)
    }
}
