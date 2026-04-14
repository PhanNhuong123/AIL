use crate::types::SourceSpan;

/// Errors produced during `.ail` text parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// AIL-P001: pest grammar rejected the input.
    #[error("AIL-P001 syntax error: {message}")]
    SyntaxError {
        message: String,
        span: Option<SourceSpan>,
    },

    /// AIL-P002: indentation is not a multiple of 2 spaces.
    #[error("AIL-P002 invalid indentation at line {}: expected multiple of 2 spaces, found {found}", span.line)]
    InvalidIndentation { found: usize, span: SourceSpan },

    /// AIL-P003: indent increases by more than 2 (skipped a level).
    #[error("AIL-P003 indent jump at line {}: indent went from {parent_indent} to {child_indent}", span.line)]
    IndentJump {
        parent_indent: usize,
        child_indent: usize,
        span: SourceSpan,
    },

    /// AIL-P004: child statement appears without a structural parent.
    #[error("AIL-P004 orphan child at line {}: no parent at indent {expected_parent_indent}", span.line)]
    OrphanChild {
        expected_parent_indent: usize,
        span: SourceSpan,
    },

    /// AIL-P005: unrecognized statement rule inside statement_body.
    #[error("AIL-P005 unknown pattern at line {}: {rule_name}", span.line)]
    UnknownPattern { rule_name: String, span: SourceSpan },

    /// AIL-P006: required grammar sub-element missing from a statement.
    #[error("AIL-P006 missing element in {pattern} at line {}: {detail}", span.line)]
    MissingElement {
        pattern: String,
        detail: String,
        span: SourceSpan,
    },

    /// AIL-P007: graph assembly failed.
    #[error("AIL-P007 graph error: {0}")]
    GraphError(#[from] ail_graph::GraphError),

    /// AIL-P008: promise statement not under a Do node.
    #[error("AIL-P008 promise must be child of a Do node at line {}", span.line)]
    PromiseWithoutDo { span: SourceSpan },

    /// AIL-P009: I/O error reading `.ail` files or directories.
    #[error("AIL-P009 I/O error: {message}")]
    IoError {
        message: String,
        path: Option<String>,
    },
}
