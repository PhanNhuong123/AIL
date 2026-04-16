use std::cmp::Ordering;
use std::collections::HashMap;

use ail_graph::{GraphBackend, NodeId, Pattern, SearchResult};

use crate::errors::SearchError;
use crate::index::EmbeddingIndex;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Standard RRF constant (k = 60) from the IR literature.
///
/// Controls the smoothing between high-ranked and low-ranked documents.
/// See also review issue [10.2-A]: k=60 is the v2.0 default; tune later.
const RRF_K: f64 = 60.0;

// ─── Types ────────────────────────────────────────────────────────────────────

/// Which ranking source(s) contributed a [`HybridSearchResult`].
///
/// MCP and CLI callers can use this field to explain result order to users or AI
/// agents without re-running the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingSource {
    /// This node was returned only by BM25 keyword search.
    Bm25Only,
    /// This node was returned only by semantic (embedding) search.
    SemanticOnly,
    /// This node was returned by both BM25 and semantic search.
    Both,
}

/// A single result from hybrid (BM25 + semantic) search.
///
/// All fields needed to display and explain a result are included so callers do
/// not need to query the graph a second time.
#[derive(Debug, Clone, PartialEq)]
pub struct HybridSearchResult {
    /// The matched node.
    pub node_id: NodeId,
    /// The node's intent string.
    pub intent: String,
    /// The node's symbol name, if present (`metadata.name`).
    pub name: Option<String>,
    /// The node's pattern kind.
    pub pattern: Pattern,
    /// Depth of this node in the Ev tree (root = 0).
    pub depth: usize,
    /// Path from the root down to this node (name or first-word-of-intent per node).
    pub path: Vec<String>,
    /// Combined RRF score: `Σ 1 / (k + rank + 1)` across contributing sources.
    pub rrf_score: f64,
    /// 0-based rank in the BM25 result list, or `None` if not in BM25 results.
    pub bm25_rank: Option<usize>,
    /// 0-based rank in the semantic result list, or `None` if not in semantic results.
    pub semantic_rank: Option<usize>,
    /// Which source(s) contributed this result (use for provenance display).
    pub source: RankingSource,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Combine pre-computed BM25 results with semantic search into a single ranked list.
///
/// ## Algorithm
///
/// Uses **Reciprocal Rank Fusion** (RRF, k = 60):
/// ```text
/// score(doc) = Σ  1 / (60 + rank_in_list + 1)
/// ```
/// The sum runs across BM25 and semantic rankings. A node present in both lists
/// receives contributions from each, naturally ranking it above nodes from only
/// one source.
///
/// ## Fallback
///
/// When `embeddings` is `None` or empty the function falls back to BM25-only
/// ranking. Each BM25 result is wrapped in a [`HybridSearchResult`] with
/// `source = RankingSource::Bm25Only` and an RRF score computed from its BM25 rank
/// alone. Callers detect the fallback structurally via the `source` field.
///
/// ## Parameters
///
/// - `query` — raw query string; embedded by `embeddings.embed_query` for the
///   semantic leg.
/// - `bm25_results` — ordered BM25 results (index 0 = highest BM25 rank).
///   Produced by `Bm25Index::search`; must not be re-ranked by the caller.
/// - `embeddings` — optional embedding index; `None` triggers BM25-only fallback.
/// - `graph` — used only to enrich semantic-only hits with node metadata and path;
///   BM25 hits carry their own metadata from the BM25 result.
/// - `limit` — maximum number of results to return.
pub fn hybrid_search(
    query: &str,
    bm25_results: &[SearchResult],
    embeddings: Option<&EmbeddingIndex>,
    graph: &dyn GraphBackend,
    limit: usize,
) -> Result<Vec<HybridSearchResult>, SearchError> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    // ── Fallback: no embeddings available ─────────────────────────────────────
    let embeddings = match embeddings {
        None => return Ok(bm25_only_fallback(bm25_results, limit)),
        Some(idx) if idx.is_empty() => return Ok(bm25_only_fallback(bm25_results, limit)),
        Some(idx) => idx,
    };

    // ── Semantic leg ──────────────────────────────────────────────────────────
    let query_vec = embeddings.embed_query(query)?;
    let semantic_results = embeddings.search(&query_vec, 20)?;

    // ── RRF accumulation ──────────────────────────────────────────────────────
    // Per-node accumulators: (rrf_score, bm25_rank, semantic_rank).
    let mut accum: HashMap<NodeId, (f64, Option<usize>, Option<usize>)> = HashMap::new();

    for (rank, result) in bm25_results.iter().enumerate() {
        let entry = accum.entry(result.node_id).or_insert((0.0, None, None));
        entry.0 += 1.0 / (RRF_K + rank as f64 + 1.0);
        entry.1 = Some(rank);
    }

    for (rank, result) in semantic_results.iter().enumerate() {
        let entry = accum.entry(result.node_id).or_insert((0.0, None, None));
        entry.0 += 1.0 / (RRF_K + rank as f64 + 1.0);
        entry.2 = Some(rank);
    }

    // ── Sort: rrf_score desc, node_id string asc (tiebreak — deterministic) ──
    let mut ranked: Vec<(NodeId, f64, Option<usize>, Option<usize>)> = accum
        .into_iter()
        .map(|(id, (score, b, s))| (id, score, b, s))
        .collect();

    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.to_string().cmp(&b.0.to_string()))
    });
    ranked.truncate(limit);

    // ── Enrich with node data ─────────────────────────────────────────────────
    let mut results = Vec::with_capacity(ranked.len());
    for (node_id, rrf_score, bm25_rank, semantic_rank) in ranked {
        let source = match (bm25_rank, semantic_rank) {
            (Some(_), Some(_)) => RankingSource::Both,
            (Some(_), None) => RankingSource::Bm25Only,
            (None, Some(_)) => RankingSource::SemanticOnly,
            (None, None) => unreachable!("RRF node must appear in at least one list"),
        };

        let enriched = if let Some(rank) = bm25_rank {
            // BM25 result carries full node metadata and path already.
            let r = &bm25_results[rank];
            HybridSearchResult {
                node_id,
                intent: r.intent.clone(),
                name: r.name.clone(),
                pattern: r.pattern.clone(),
                depth: r.depth,
                path: r.path.clone(),
                rrf_score,
                bm25_rank,
                semantic_rank,
                source,
            }
        } else {
            // Semantic-only: look up node data from graph.
            enrich_from_graph(node_id, rrf_score, semantic_rank, source, graph)?
        };
        results.push(enriched);
    }

    Ok(results)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Wrap BM25 results into `HybridSearchResult` with single-list RRF scores.
fn bm25_only_fallback(bm25_results: &[SearchResult], limit: usize) -> Vec<HybridSearchResult> {
    bm25_results
        .iter()
        .enumerate()
        .take(limit)
        .map(|(rank, r)| HybridSearchResult {
            node_id: r.node_id,
            intent: r.intent.clone(),
            name: r.name.clone(),
            pattern: r.pattern.clone(),
            depth: r.depth,
            path: r.path.clone(),
            rrf_score: 1.0 / (RRF_K + rank as f64 + 1.0),
            bm25_rank: Some(rank),
            semantic_rank: None,
            source: RankingSource::Bm25Only,
        })
        .collect()
}

/// Look up node metadata from the graph and build a path by walking to the root.
fn enrich_from_graph(
    node_id: NodeId,
    rrf_score: f64,
    semantic_rank: Option<usize>,
    source: RankingSource,
    graph: &dyn GraphBackend,
) -> Result<HybridSearchResult, SearchError> {
    let node = graph
        .get_node(node_id)
        .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?
        .ok_or_else(|| {
            SearchError::EmbeddingBuildFailed(format!(
                "semantic result node {node_id} not found in graph"
            ))
        })?;

    let depth = graph
        .depth(node_id)
        .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?;

    let path = build_path(node_id, graph)?;

    Ok(HybridSearchResult {
        node_id,
        intent: node.intent.clone(),
        name: node.metadata.name.clone(),
        pattern: node.pattern.clone(),
        depth,
        path,
        rrf_score,
        bm25_rank: None,
        semantic_rank,
        source,
    })
}

/// Walk the Ev parent chain to the root and return the path (root first).
fn build_path(node_id: NodeId, graph: &dyn GraphBackend) -> Result<Vec<String>, SearchError> {
    let mut path: Vec<String> = Vec::new();
    let mut current_id = node_id;

    loop {
        let node = graph
            .get_node(current_id)
            .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?
            .ok_or_else(|| {
                SearchError::EmbeddingBuildFailed(format!(
                    "node {current_id} missing during path walk"
                ))
            })?;

        let label = node
            .metadata
            .name
            .clone()
            .unwrap_or_else(|| first_word(&node.intent));
        path.push(label);

        match graph
            .parent(current_id)
            .map_err(|e| SearchError::EmbeddingBuildFailed(e.to_string()))?
        {
            Some(parent_id) => current_id = parent_id,
            None => break,
        }
    }

    path.reverse();
    Ok(path)
}

fn first_word(s: &str) -> String {
    s.split_whitespace().next().unwrap_or(s).to_string()
}
