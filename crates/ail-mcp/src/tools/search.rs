//! `ail.search` tool — hybrid (BM25 + semantic) search with a lazy cached index.
//!
//! The BM25 leg is cached between calls and reset on graph reload. When the
//! `embeddings` feature is enabled and a compatible `.ail.db` exists in
//! `project_root`, the embedding index is loaded once from SQLite and used for
//! RRF fusion. Without embeddings the BM25-only fallback path still populates
//! provenance fields (`source`, `rrf_score`, `bm25_rank`, `semantic_rank`).

use std::cell::RefCell;

use ail_graph::{AilGraph, Bm25Index};
use ail_search::{hybrid_search, EmbeddingIndex, RankingSource};

use ail_graph::SearchResult;

use crate::types::tool_io::{SearchInput, SearchItem, SearchOutput};

const DEFAULT_BUDGET: usize = 10;

/// Map raw BM25 results to `SearchItem` DTOs for the BM25-only fallback path.
///
/// Used when hybrid search is unavailable or fails, so keyword results are never
/// silently lost. RRF scores use the standard k=60 constant.
fn bm25_fallback_items(bm25_raw: &[SearchResult], budget: usize) -> Vec<SearchItem> {
    bm25_raw
        .iter()
        .take(budget)
        .enumerate()
        .map(|(rank, r)| SearchItem {
            node_id: r.node_id.to_string(),
            score: r.score,
            intent: r.intent.clone(),
            pattern: format!("{:?}", r.pattern),
            path: r.path.clone(),
            source: "bm25_only".to_string(),
            rrf_score: 1.0 / (60.0 + rank as f64 + 1.0),
            bm25_rank: Some(rank),
            semantic_rank: None,
        })
        .collect()
}

/// Execute a hybrid search against the project graph.
///
/// When `embeddings` is `Some`, RRF fusion is performed and results may carry
/// `source = "both"` or `source = "semantic_only"`. When `None`, the BM25-only
/// fallback path is used and all results carry `source = "bm25_only"`.
///
/// Callers must clear `cache` whenever the underlying graph changes.
pub(crate) fn run_search(
    graph: &AilGraph,
    cache: &RefCell<Option<Bm25Index>>,
    embeddings: Option<&EmbeddingIndex>,
    input: &SearchInput,
) -> SearchOutput {
    // Build or reuse the BM25 index.
    let mut borrow = cache.borrow_mut();
    if borrow.is_none() {
        *borrow = Some(Bm25Index::build_from_graph(graph));
    }
    let index = borrow.as_ref().unwrap(); // safe: just set above

    let budget = input.budget.unwrap_or(DEFAULT_BUDGET);
    let bm25_raw = index.search(&input.query, budget, graph);

    let hybrid = match hybrid_search(&input.query, &bm25_raw, embeddings, graph, budget) {
        Ok(results) => results,
        Err(_) => {
            // Hybrid search failed (stale embeddings, dimension mismatch, etc.).
            // Fall back to BM25-only results so keyword matches are never lost.
            let fallback = bm25_fallback_items(&bm25_raw, budget);
            let total = fallback.len();
            return SearchOutput {
                results: fallback,
                total,
            };
        }
    };

    let results: Vec<SearchItem> = hybrid
        .into_iter()
        .map(|r| {
            let source = match r.source {
                RankingSource::Bm25Only => "bm25_only",
                RankingSource::SemanticOnly => "semantic_only",
                RankingSource::Both => "both",
            };
            SearchItem {
                node_id: r.node_id.to_string(),
                score: r.rrf_score as f32,
                intent: r.intent,
                pattern: format!("{:?}", r.pattern),
                path: r.path,
                source: source.to_string(),
                rrf_score: r.rrf_score,
                bm25_rank: r.bm25_rank,
                semantic_rank: r.semantic_rank,
            }
        })
        .collect();

    let total = results.len();
    SearchOutput { results, total }
}

/// Attempt to load a hybrid embedding index from the project database.
///
/// Returns `None` if:
/// - No `.ail.db` file is found in `project_root`.
/// - The DB has no stored vectors or the stored model differs from the current
///   ONNX model (triggers a reindex).
/// - The ONNX model files are absent from `~/.ail/models/`.
/// - Any IO or inference error occurs during loading.
///
/// Errors are silently discarded; the caller falls back to BM25-only search.
#[cfg(feature = "embeddings")]
pub(crate) fn try_load_embedding_index(project_root: &std::path::Path) -> Option<EmbeddingIndex> {
    use ail_db::{EmbeddingModelStatus, SqliteGraph};
    use ail_search::{OnnxEmbeddingProvider, DEFAULT_MODEL_NAME, ONNX_DIMENSION};

    let db_path = find_db(project_root)?;
    let db = SqliteGraph::open(&db_path).ok()?;

    // Only use cached vectors when stored model, provider, and dimensions match.
    let provider = DEFAULT_MODEL_NAME.split('/').next().unwrap_or("unknown");
    match db
        .check_embedding_metadata(DEFAULT_MODEL_NAME, provider, ONNX_DIMENSION)
        .ok()?
    {
        EmbeddingModelStatus::Compatible => {}
        _ => return None,
    }

    let vectors = db.load_all_embeddings().ok()?;
    if vectors.is_empty() {
        return None;
    }

    let model_dir = OnnxEmbeddingProvider::ensure_model().ok()?;
    let provider = OnnxEmbeddingProvider::new(&model_dir).ok()?;
    Some(EmbeddingIndex::from_vectors(Box::new(provider), vectors))
}

/// Locate the project `.ail.db` file: checks `project.ail.db` first, then
/// scans the directory for any `*.ail.db` file.
#[cfg(feature = "embeddings")]
fn find_db(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let conventional = root.join("project.ail.db");
    if conventional.exists() {
        return Some(conventional);
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("db")
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with(".ail"))
                .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}
