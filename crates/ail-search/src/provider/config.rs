use std::fmt;
use std::path::PathBuf;

/// Configuration for the embedding provider used by `ail-search`.
///
/// Passed by callers (CLI `ail search --setup`, MCP loader) to choose between
/// local ONNX inference, a cloud API, and BM25-only fallback. Using `None`
/// means `hybrid_search` falls back to BM25 ranking with
/// `RankingSource::Bm25Only`.
#[derive(Clone, PartialEq)]
pub enum SearchProviderConfig {
    /// No embedding provider — `hybrid_search` falls back to BM25-only.
    None,
    /// Local ONNX inference via [`OnnxEmbeddingProvider`].
    ///
    /// `model_dir` must contain `model.onnx` and `tokenizer.json` from
    /// `https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2`.
    ///
    /// [`OnnxEmbeddingProvider`]: crate::provider::OnnxEmbeddingProvider
    LocalOnnx {
        /// Directory containing `model.onnx` and `tokenizer.json`.
        model_dir: PathBuf,
    },
    /// Cloud embedding via the OpenAI embedding API.
    ///
    /// Config-only — actual HTTP calls are deferred to the hardening task.
    /// At runtime, `api_key` must be populated from `OPENAI_API_KEY`.
    OpenAi {
        /// OpenAI API key for embedding requests.
        api_key: String,
    },
}

impl fmt::Debug for SearchProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::LocalOnnx { model_dir } => f
                .debug_struct("LocalOnnx")
                .field("model_dir", model_dir)
                .finish(),
            Self::OpenAi { .. } => f
                .debug_struct("OpenAi")
                .field("api_key", &"***REDACTED***")
                .finish(),
        }
    }
}
