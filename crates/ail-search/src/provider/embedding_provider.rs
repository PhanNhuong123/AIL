use crate::errors::SearchError;

/// Provider-agnostic interface for computing dense embedding vectors.
///
/// Implementations must be `Send + Sync` so they can be stored behind `Arc`
/// or `Box<dyn EmbeddingProvider>` in multi-threaded contexts (e.g., MCP
/// server handling concurrent requests).
///
/// ## Extension pattern
///
/// Callers that need a no-op or test-only provider can implement the trait
/// with a small inline struct — only `embed` and `dimension` are required.
pub trait EmbeddingProvider: Send + Sync {
    /// Compute an embedding for a single piece of text.
    ///
    /// Returns a `Vec<f32>` of length [`dimension`]. The vector is
    /// L2-normalised so that cosine similarity equals the dot product.
    fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError>;

    /// Compute embeddings for a batch of texts.
    ///
    /// The default implementation calls [`embed`] once per text. Providers
    /// with native batch APIs (e.g., `OnnxEmbeddingProvider`) should override
    /// this for efficiency.
    ///
    /// Returns one `Vec<f32>` per input text, in the same order as `texts`.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SearchError> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Number of dimensions in each embedding vector (constant per provider).
    ///
    /// For `all-MiniLM-L6-v2` this is 384.
    fn dimension(&self) -> usize;

    /// Short display name for logging and index version tracking.
    ///
    /// Format: `"backend/model-name"`, e.g. `"onnx/all-MiniLM-L6-v2"`.
    fn name(&self) -> &str;
}
