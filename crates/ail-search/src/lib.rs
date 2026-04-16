//! `ail-search` — Embedding provider interface for semantic search in AIL.
//!
//! This crate defines the [`EmbeddingProvider`] trait and the local ONNX-backed
//! implementation ([`OnnxEmbeddingProvider`]) for `all-MiniLM-L6-v2`.
//!
//! ## Feature flags
//!
//! | Feature | What it enables |
//! |---------|-----------------|
//! | *(none, default)* | `EmbeddingProvider` trait + `SearchError` only |
//! | `embeddings` | `OnnxEmbeddingProvider` via ONNX Runtime |
//!
//! Without the `embeddings` feature the crate compiles with zero ONNX
//! dependencies. Callers (CLI, MCP) can depend on `ail-search` cheaply and
//! fall back to BM25-only search when `EmbeddingProvider` is unavailable.
//!
//! ## Quick start (with `embeddings` feature)
//!
//! ```rust,ignore
//! // Requires the `embeddings` Cargo feature.
//! use ail_search::{EmbeddingProvider, OnnxEmbeddingProvider};
//!
//! let model_dir = OnnxEmbeddingProvider::ensure_model().expect("model present");
//! let provider = OnnxEmbeddingProvider::new(&model_dir).expect("loaded");
//! let vec = provider.embed("transfer money safely").expect("embed ok");
//! assert_eq!(vec.len(), 384);
//! ```

pub mod errors;
pub mod provider;

pub use errors::SearchError;
pub use provider::EmbeddingProvider;

#[cfg(feature = "embeddings")]
pub use provider::OnnxEmbeddingProvider;
