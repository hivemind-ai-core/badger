use crate::source::{FileId, SourceMap, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable diagnostic code such as `E0201`.
    pub code: Option<&'static str>,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    /// Expected and actual shapes (types, effects, field sets).
    pub expected: Option<String>,
    pub actual: Option<String>,
    /// Readable path of the enclosing definition (`fn app.main`).
    pub scope: Option<String>,
    /// Related definitions: a name and where it is declared.
    pub related: Vec<(String, Span)>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            labels: vec![],
            notes: vec![],
            expected: None,
            actual: None,
            scope: None,
            related: vec![],
        }
    }

    /// Record the expected and actual shapes of a mismatch.
    pub fn with_shapes(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn with_related(mut self, name: impl Into<String>, span: Span) -> Self {
        self.related.push((name.into(), span));
        self
    }

    /// The primary span (first non-synthetic label).
    pub fn primary(&self) -> Option<Span> {
        self.labels.iter().map(|l| l.span).find(|s| !s.is_dummy())
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, ..Self::error(message) }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self { severity: Severity::Note, ..Self::error(message) }
    }

    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = Some(code);
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label { span, message: message.into() });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Accumulates diagnostics across phases; phases keep going after errors
/// where they can so users see as many problems as possible per run.
#[derive(Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Diagnostic> {
        self.items.iter_mut()
    }

    /// Drop repeated diagnostics (same code, message, and source position),
    /// e.g. from checking modules shared by several entry points.
    pub fn dedupe(&mut self, sources: &SourceMap) {
        let mut seen = std::collections::HashSet::new();
        self.items.retain(|d| {
            let pos = d.primary().map(|s| (sources.get(s.file).name.clone(), s.start, s.end));
            seen.insert((d.code, d.message.clone(), pos))
        });
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    pub fn extend(&mut self, ds: impl IntoIterator<Item = Diagnostic>) {
        self.items.extend(ds);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn error_count(&self) -> usize {
        self.items.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    /// Render all diagnostics with ariadne to `out`.
    pub fn render(&self, sources: &SourceMap, out: &mut impl std::io::Write) -> std::io::Result<()> {
        self.render_with(sources, out, true)
    }

    pub fn render_with(
        &self,
        sources: &SourceMap,
        out: &mut impl std::io::Write,
        color: bool,
    ) -> std::io::Result<()> {
        use ariadne::{Color, Config, Report, ReportKind};

        for d in &self.items {
            let kind = match d.severity {
                Severity::Error => ReportKind::Error,
                Severity::Warning => ReportKind::Warning,
                Severity::Note => ReportKind::Advice,
            };
            let anchor = d.labels.iter().map(|l| l.span).find(|s| !s.is_dummy());
            let (file, range) = match anchor {
                Some(s) => (Some(s.file), s.range()),
                None => (None, 0..0),
            };
            let key = |f: FileId| sources.get(f).name.clone();
            let mut report =
                Report::build(kind, (file.map(key).unwrap_or_default(), range)).with_message(&d.message);
            if let Some(code) = d.code {
                report = report.with_code(code);
            }
            report = report.with_config(Config::default().with_color(color));
            let mut notes: Vec<String> = vec![];
            if let (Some(e), Some(a)) = (&d.expected, &d.actual) {
                notes.push(format!("expected: {e}\n   found: {a}"));
            }
            if let Some(sc) = &d.scope {
                notes.push(format!("in {sc}"));
            }
            for (name, sp) in &d.related {
                if !sp.is_dummy() {
                    let f = sources.get(sp.file);
                    let (line, col) = f.line_col(sp.start as usize);
                    notes.push(format!("related: `{name}` declared at {}:{line}:{col}", f.name));
                }
            }
            notes.extend(d.notes.iter().cloned());
            if anchor.is_none() {
                let code = d.code.map(|c| format!("[{c}] ")).unwrap_or_default();
                writeln!(out, "{kind:?}: {code}{}", d.message)?;
                for n in &notes {
                    writeln!(out, "  note: {n}")?;
                }
                continue;
            }
            for l in d.labels.iter().filter(|l| !l.span.is_dummy()) {
                report = report.with_label(
                    ariadne::Label::new((key(l.span.file), l.span.range()))
                        .with_message(&l.message)
                        .with_color(Color::Red),
                );
            }
            for n in &notes {
                report = report.with_note(n);
            }
            let cache = ariadne::sources(d.labels.iter().filter(|l| !l.span.is_dummy()).map(|l| {
                let f = sources.get(l.span.file);
                (f.name.clone(), f.text.clone())
            }));
            report.finish().write(cache, &mut *out)?;
        }
        Ok(())
    }
}

impl Diagnostics {
    /// Render diagnostics as a JSON array (one object per diagnostic) with
    /// stable codes, severities, messages, and 1-based line/column spans.
    pub fn render_json(&self, sources: &SourceMap) -> String {
        let mut out = String::from("[");
        for (i, d) in self.items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Note => "note",
            };
            out.push_str(&format!(
                "{{\"code\":{},\"severity\":\"{sev}\",\"message\":{},\"labels\":[",
                d.code.map(json_str).unwrap_or_else(|| "null".into()),
                json_str(&d.message)
            ));
            for (j, l) in d.labels.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                let f = sources.get(l.span.file);
                let (line, col) = f.line_col(l.span.start as usize);
                let (eline, ecol) = f.line_col(l.span.end as usize);
                out.push_str(&format!(
                    "{{\"file\":{},\"module\":{},\"start\":{{\"line\":{line},\"column\":{col}}},\"end\":{{\"line\":{eline},\"column\":{ecol}}},\"message\":{}}}",
                    json_str(&f.name),
                    f.module.as_deref().map(json_str).unwrap_or_else(|| "null".into()),
                    json_str(&l.message)
                ));
            }
            out.push_str("],\"notes\":[");
            for (j, n) in d.notes.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str(&json_str(n));
            }
            let opt = |v: &Option<String>| v.as_deref().map(json_str).unwrap_or_else(|| "null".into());
            out.push_str(&format!(
                "],\"expected\":{},\"actual\":{},\"scope\":{},\"related\":[",
                opt(&d.expected),
                opt(&d.actual),
                opt(&d.scope)
            ));
            for (j, (name, sp)) in d.related.iter().filter(|(_, sp)| !sp.is_dummy()).enumerate() {
                if j > 0 {
                    out.push(',');
                }
                let f = sources.get(sp.file);
                let (line, col) = f.line_col(sp.start as usize);
                out.push_str(&format!(
                    "{{\"name\":{},\"file\":{},\"module\":{},\"line\":{line},\"column\":{col}}}",
                    json_str(name),
                    json_str(&f.name),
                    f.module.as_deref().map(json_str).unwrap_or_else(|| "null".into())
                ));
            }
            out.push_str("]}");
        }
        out.push(']');
        out
    }
}

pub fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rfind('\n').map(|p| offset - p).unwrap_or(offset + 1);
    (line, col)
}

pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
