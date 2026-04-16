use std::path::PathBuf;

/// Errors produced by `ail-search` operations.
///
/// Error codes follow the `AIL-S0xx` range (cross-phase convention [X-4]).
///
/// All variants are **recoverable**: callers may catch any `SearchError` and
/// fall back to BM25-only search without losing search availability.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// AIL-S001 — Embedding model files are absent from the expected location.
    ///
    /// The `hint` field contains an actionable message telling the user how to
    /// install the model or run `ail search --setup`.
    #[error("AIL-S001: model not found at {path}: {hint}")]
    ModelNotFound { path: PathBuf, hint: String },

    /// AIL-S002 — The ONNX session or tokenizer could not be loaded.
    #[error("AIL-S002: model load failed: {0}")]
    ModelLoadFailed(String),

    /// AIL-S003 — The ONNX runtime returned an error during inference.
    #[error("AIL-S003: inference failed: {0}")]
    InferenceFailed(String),

    /// AIL-S004 — The tokenizer failed to encode input text.
    #[error("AIL-S004: tokenization failed: {0}")]
    TokenizationFailed(String),

    /// AIL-S005 — Graph query failed during embedding index construction.
    #[error("AIL-S005: embedding index build failed: {0}")]
    EmbeddingBuildFailed(String),

    /// AIL-S006 — Query vector dimension does not match the indexed vector dimension.
    ///
    /// Occurs when the embedding provider is swapped after index construction, or
    /// when a caller passes a query vector from a different model.
    #[error("AIL-S006: dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Underlying I/O error (e.g., reading model files from disk).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
