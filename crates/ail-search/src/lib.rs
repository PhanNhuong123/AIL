//! `ail-search` — Hybrid (BM25 + semantic) search for AIL.
//!
//! This crate provides:
//! - [`EmbeddingProvider`] — provider-agnostic embedding interface.
//! - [`EmbeddingIndex`] — in-memory index of node embedding vectors.
//! - [`hybrid_search`] — RRF fusion of BM25 and semantic results.
//! - [`HybridSearchResult`] / [`RankingSource`] — stable result types with
//!   provenance metadata for MCP and CLI callers.
//! - Helper functions: [`cosine_similarity`], [`node_embedding_text`].
//!
//! ## Feature flags
//!
//! | Feature | What it enables |
//! |---------|-----------------|
//! | *(none, default)* | Everything above; zero ONNX dependencies |
//! | `embeddings` | `OnnxEmbeddingProvider` via ONNX Runtime |
//!
//! Without the `embeddings` feature the crate compiles with zero ONNX
//! dependencies. Callers can depend on `ail-search` cheaply and use
//! `hybrid_search` with a custom [`EmbeddingProvider`] or fall back to
//! BM25-only mode via `embeddings: None`.
//!
//! ## Quick start — hybrid search
//!
//! ```rust,ignore
//! use ail_search::{EmbeddingIndex, hybrid_search};
//!
//! // 1. Build embedding index (requires a provider + GraphBackend).
//! let index = EmbeddingIndex::build(&graph, provider).expect("built");
//!
//! // 2. Run BM25 first (caller-controlled).
//! let bm25 = Bm25Index::build_from_graph(&graph);
//! let bm25_results = bm25.search(query, 20, &graph);
//!
//! // 3. Fuse with hybrid search.
//! let results = hybrid_search(query, &bm25_results, Some(&index), &graph, 10).unwrap();
//! ```

pub mod errors;
pub mod provider;

mod hybrid;
mod index;

pub use errors::SearchError;
pub use hybrid::{hybrid_search, HybridSearchResult, RankingSource};
pub use index::{cosine_similarity, node_embedding_text, EmbeddingIndex, SemanticResult};
pub use provider::EmbeddingProvider;

#[cfg(feature = "embeddings")]
pub use provider::OnnxEmbeddingProvider;
