use la_arena::{Arena, Idx};

pub type FileId = Idx<SourceFile>;

#[derive(Debug)]
pub struct SourceFile {
    pub name: String,
    pub text: String,
    /// Dotted module path of this file (`app.orders`, `std.io`), when known.
    pub module: Option<String>,
    /// Byte offsets where lines start, built on first use.
    lines: std::sync::OnceLock<Vec<u32>>,
}

impl SourceFile {
    /// 1-based line and column (in bytes) of byte `offset`.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let lines = self.lines.get_or_init(|| {
            std::iter::once(0).chain(self.text.match_indices('\n').map(|(i, _)| i as u32 + 1)).collect()
        });
        let line = lines.partition_point(|&s| s as usize <= offset);
        (line, offset - lines[line - 1] as usize + 1)
    }
}

/// Owns every source text the compiler has loaded.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Arena<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        self.files.alloc(SourceFile {
            name: name.into(),
            text: text.into(),
            module: None,
            lines: Default::default(),
        })
    }

    pub fn set_module(&mut self, id: FileId, module: String) {
        self.files[id].module = Some(module);
    }

    pub fn get(&self, id: FileId) -> &SourceFile {
        &self.files[id]
    }
}

/// Byte range into a single source file. Kept `Copy` and small (12 bytes)
/// because every AST/HIR node carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(file: FileId, start: usize, end: usize) -> Self {
        Self { file, start: start as u32, end: end as u32 }
    }

    /// A span that points nowhere (synthetic code).
    pub fn dummy() -> Span {
        Span { file: la_arena::Idx::from_raw(u32::MAX.into()), start: 0, end: 0 }
    }

    pub fn is_dummy(self) -> bool {
        self.file == la_arena::Idx::from_raw(u32::MAX.into())
    }

    pub fn to(self, other: Span) -> Span {
        if self.file != other.file {
            return self;
        }
        Span { file: self.file, start: self.start.min(other.start), end: self.end.max(other.end) }
    }

    pub fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn indexed_line_col_matches_scan() {
        let mut m = super::SourceMap::default();
        let text = "ab\n\ncdé\nlast";
        let f = m.add("t", text);
        for off in (0..=text.len() + 2).filter(|o| *o > text.len() || text.is_char_boundary(*o)) {
            assert_eq!(m.get(f).line_col(off), crate::diagnostics::line_col(text, off), "offset {off}");
        }
    }
}
