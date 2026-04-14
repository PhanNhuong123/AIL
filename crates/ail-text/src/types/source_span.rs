/// Byte-level source location for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub col: usize,
    /// Byte offset from start of source.
    pub offset: usize,
    /// Length in bytes of the spanned text.
    pub len: usize,
}

impl SourceSpan {
    /// Build a `SourceSpan` from a pest `Span`.
    pub fn from_pest_span(span: pest::Span<'_>) -> Self {
        let (line, col) = span.start_pos().line_col();
        Self {
            line,
            col,
            offset: span.start(),
            len: span.end() - span.start(),
        }
    }
}
