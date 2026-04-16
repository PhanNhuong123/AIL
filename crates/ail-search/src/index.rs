use std::cmp::Ordering;
use std::collections::HashMap;

use ail_graph::{GraphBackend, Node, NodeId};

use crate::errors::SearchError;
use crate::provider::EmbeddingProvider;

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Compute cosine similarity between two embedding vectors.
///
/// Both vectors are expected to be L2-normalised (as produced by
/// `OnnxEmbeddingProvider`), but the function works for un-normalised vectors too.
/// Returns `0.0` when either vector is a zero vector.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Canonical text representation of a node for embedding.
///
/// Both [`EmbeddingIndex::build`] and [`EmbeddingIndex::update_node`] call this
/// function so the text construction is a single source of truth.
///
/// Format:
/// - Leaf node with non-empty expression: `"<intent> <expression>"`
/// - All other nodes: `"<intent>"`
pub fn node_embedding_text(node: &Node) -> String {
    match &node.expression {
        Some(expr) if !expr.0.is_empty() => format!("{} {}", node.intent, expr),
        _ => node.intent.clone(),
    }
}

// ─── SemanticResult ───────────────────────────────────────────────────────────

/// A single result from a semantic (embedding-based) search.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticResult {
    /// The matched node.
    pub node_id: NodeId,
    /// Cosine similarity score as f64 (higher = more similar).
    pub score: f64,
}

// ─── EmbeddingIndex ───────────────────────────────────────────────────────────

/// In-memory index of node embedding vectors.
///
/// Build with [`EmbeddingIndex::build`] (from a [`GraphBackend`]) or
/// [`EmbeddingIndex::from_vectors`] (for testing or SQLite-loaded vectors in Task 10.3).
///
/// The `provider` field is private; use [`embed_query`] to embed a query string.
///
/// [`embed_query`]: EmbeddingIndex::embed_query
pub struct EmbeddingIndex {
    provider: Box<dyn EmbeddingProvider>,
    vectors: HashMap<NodeId, Vec<f32>>,
}

impl EmbeddingIndex {
    /// Build an embedding index by computing vectors for every node in `graph`.
    ///
    /// Uses [`node_embedding_text`] for text construction. Nodes with an empty
    /// intent are skipped — they would produce a near-zero vector and poison
    /// cosine rankings.
    ///
    /// # Errors
    ///
    /// Returns [`SearchError::EmbeddingBuildFailed`] if the graph cannot be
    /// queried. Returns [`SearchError::InferenceFailed`] / [`SearchError::TokenizationFailed`]
    /// if the provider fails to embed any text.
    pub fn build(
        graph: &dyn GraphBackend,
        provider: Box<dyn EmbeddingProvider>,
    ) -> Result<Self, SearchError> {
        let node_ids = graph
            .all_node_ids()
            .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?;

        let mut id_text_pairs: Vec<(NodeId, String)> = Vec::with_capacity(node_ids.len());

        for id in node_ids {
            let node = graph
                .get_node(id)
                .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?;

            if let Some(node) = node {
                if node.intent.trim().is_empty() {
                    #[cfg(debug_assertions)]
                    eprintln!("[ail-search] skipping node {id}: empty intent");
                    continue;
                }
                id_text_pairs.push((id, node_embedding_text(&node)));
            }
        }

        let text_refs: Vec<&str> = id_text_pairs.iter().map(|(_, t)| t.as_str()).collect();
        let vecs = provider.embed_batch(&text_refs)?;

        let vectors = id_text_pairs
            .into_iter()
            .zip(vecs)
            .map(|((id, _), v)| (id, v))
            .collect();

        Ok(Self { provider, vectors })
    }

    /// Construct an index directly from a pre-built vector map.
    ///
    /// This is useful for:
    /// - Tests — inject controlled vectors without an ONNX model.
    /// - Task 10.3 — bulk-load persisted vectors from SQLite.
    pub fn from_vectors(
        provider: Box<dyn EmbeddingProvider>,
        vectors: HashMap<NodeId, Vec<f32>>,
    ) -> Self {
        Self { provider, vectors }
    }

    /// Embed a query string using this index's provider.
    ///
    /// The returned vector can be passed directly to [`search`].
    ///
    /// [`search`]: EmbeddingIndex::search
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>, SearchError> {
        self.provider.embed(query)
    }

    /// Return the top-`limit` nodes ranked by cosine similarity to `query_vec`.
    ///
    /// # Errors
    ///
    /// Returns [`SearchError::DimensionMismatch`] if any stored vector has a
    /// different dimension than `query_vec`. This indicates the index was built
    /// with a different model than the one used to embed the query.
    pub fn search(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> Result<Vec<SemanticResult>, SearchError> {
        let mut scores: Vec<(NodeId, f32)> = Vec::with_capacity(self.vectors.len());

        for (&id, vec) in &self.vectors {
            if vec.len() != query_vec.len() {
                return Err(SearchError::DimensionMismatch {
                    expected: vec.len(),
                    actual: query_vec.len(),
                });
            }
            scores.push((id, cosine_similarity(query_vec, vec)));
        }

        scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
        });
        scores.truncate(limit);

        Ok(scores
            .into_iter()
            .map(|(id, score)| SemanticResult {
                node_id: id,
                score: score as f64,
            })
            .collect())
    }

    /// Recompute and update the embedding vector for `node`.
    ///
    /// Uses [`node_embedding_text`] for text construction — the same function
    /// that [`build`] uses — so the representation is always consistent.
    ///
    /// [`build`]: EmbeddingIndex::build
    pub fn update_node(&mut self, node: &Node) -> Result<(), SearchError> {
        let text = node_embedding_text(node);
        let vec = self.provider.embed(&text)?;
        self.vectors.insert(node.id, vec);
        Ok(())
    }

    /// Remove `node_id` from the index. No-op if the node is not indexed.
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.vectors.remove(&node_id);
    }

    /// Return `true` if the index contains no vectors.
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Return the number of indexed nodes.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }
}
