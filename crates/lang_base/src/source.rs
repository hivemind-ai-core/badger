use la_arena::{Arena, Idx};

pub type FileId = Idx<SourceFile>;

#[derive(Debug)]
pub struct SourceFile {
    pub name: String,
    pub text: String,
    /// Dotted module path of this file (`app.orders`, `std.io`), when known.
    pub module: Option<String>,
}

/// Owns every source text the compiler has loaded.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Arena<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        self.files.alloc(SourceFile { name: name.into(), text: text.into(), module: None })
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
