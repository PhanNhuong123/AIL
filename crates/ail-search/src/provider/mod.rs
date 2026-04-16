mod config;
mod embedding_provider;
pub use config::SearchProviderConfig;
pub use embedding_provider::EmbeddingProvider;

#[cfg(feature = "embeddings")]
mod onnx;
#[cfg(feature = "embeddings")]
pub use onnx::{OnnxEmbeddingProvider, DEFAULT_MODEL_NAME, DIMENSION as ONNX_DIMENSION};
