mod embedding_provider;
pub use embedding_provider::EmbeddingProvider;

#[cfg(feature = "embeddings")]
mod onnx;
#[cfg(feature = "embeddings")]
pub use onnx::{OnnxEmbeddingProvider, DEFAULT_MODEL_NAME};
