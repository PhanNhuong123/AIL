//! `ail.review` tool — semantic coverage review for a single graph node.
//!
//! With the `embeddings` feature active, `handle_review` lazily initialises an
//! [`OnnxEmbeddingProvider`] (same approach as `ail.search`) and calls
//! `compute_coverage` from `ail-coverage`. Without the feature, or when the
//! provider cannot be initialised, it returns a structured `"Unavailable"`
//! response instead of a JSON-RPC error, so callers always receive valid JSON.

use ail_graph::{AilGraph, GraphBackend, NodeId};

#[cfg(feature = "embeddings")]
use crate::types::tool_io::{ChildCoverageItem, MissingItem};
use crate::types::tool_io::{ReviewInput, ReviewOutput};

// ── Node resolution ──────────────────────────────────────────────────────────

/// Resolve `spec` to a `NodeId`.
///
/// Tries to parse the string as a UUID first (checking the node exists); on
/// failure, searches the graph for a node whose `metadata.name` matches
/// (first match wins).
fn resolve_node_id(graph: &AilGraph, spec: &str) -> Option<NodeId> {
    // 1. UUID parse — AilGraph::get_node returns Result<&Node, _>.
    if let Ok(id) = spec.parse::<NodeId>() {
        if graph.get_node(id).is_ok() {
            return Some(id);
        }
    }
    // 2. Name lookup via GraphBackend::find_by_name.
    graph
        .find_by_name(spec)
        .ok()
        .and_then(|ids| ids.into_iter().next())
}

// ── Suggestion builder ───────────────────────────────────────────────────────

/// Derive a human-readable action suggestion from a status label and the
/// ordered list of missing concepts.  Only called from the embeddings path.
#[cfg(feature = "embeddings")]
fn build_suggestion(status: &str, missing: &[MissingItem]) -> String {
    match status {
        "Full" => "Coverage is strong. No action needed.".to_owned(),
        "Partial" => {
            let concepts: Vec<&str> = missing.iter().take(2).map(|m| m.concept.as_str()).collect();
            if concepts.is_empty() {
                "Consider decomposing further to improve coverage.".to_owned()
            } else {
                format!("Consider addressing: {}", concepts.join(" and "))
            }
        }
        "Weak" => {
            let concepts: Vec<&str> = missing.iter().take(3).map(|m| m.concept.as_str()).collect();
            if concepts.is_empty() {
                "Weak coverage — decompose further.".to_owned()
            } else {
                format!(
                    "Weak coverage — decompose further. Top missing aspects: {}",
                    concepts.join(", ")
                )
            }
        }
        "N/A" => "Leaf node — coverage not applicable.".to_owned(),
        _ => {
            // "Unavailable" and any future unknown status.
            "Embeddings not available. Run `ail reindex --embeddings` first.".to_owned()
        }
    }
}

// ── Unavailable helper ───────────────────────────────────────────────────────

fn unavailable_response(node_id: String, node_name: Option<String>, reason: &str) -> ReviewOutput {
    ReviewOutput {
        node_id,
        node_name,
        coverage: None,
        status: "Unavailable".to_owned(),
        children_coverage: vec![],
        missing: vec![],
        suggestion: reason.to_owned(),
        empty_parent: None,
        degenerate_basis_fallback: None,
    }
}

// ── Main handler ─────────────────────────────────────────────────────────────

/// Attempt to build an [`OnnxEmbeddingProvider`] from the on-disk model files.
///
/// Returns `None` when model files are absent or loading fails.  The result is
/// cached by `McpServer` so this expensive call runs at most once per session.
#[cfg(feature = "embeddings")]
pub(crate) fn try_build_provider() -> Option<std::sync::Arc<ail_search::OnnxEmbeddingProvider>> {
    use ail_search::OnnxEmbeddingProvider;

    let model_dir = OnnxEmbeddingProvider::ensure_model().ok()?;
    let provider = OnnxEmbeddingProvider::new(&model_dir).ok()?;
    Some(std::sync::Arc::new(provider))
}

/// Handle an `ail.review` call.
///
/// On any embedding error, returns a structured `"Unavailable"` response
/// rather than propagating as a `McpError`. This mirrors the BM25 fallback
/// pattern in `ail.search`.
///
/// With the `embeddings` feature, an optional pre-built provider is accepted so
/// the caller (`McpServer`) can cache the expensive ONNX load across calls.
#[cfg(feature = "embeddings")]
pub(crate) fn handle_review(
    graph: &AilGraph,
    input: ReviewInput,
    provider: Option<&ail_search::OnnxEmbeddingProvider>,
) -> ReviewOutput {
    // 1. Resolve node.
    let node_id = match resolve_node_id(graph, &input.node) {
        Some(id) => id,
        None => {
            return unavailable_response(
                input.node.clone(),
                None,
                &format!("Node not found: \"{}\"", input.node),
            );
        }
    };

    let node_name = match graph.get_node(node_id) {
        Ok(n) => n.metadata.name.clone(),
        Err(_) => {
            return unavailable_response(
                node_id.to_string(),
                None,
                &format!("Node not found: \"{}\"", input.node),
            );
        }
    };

    handle_review_with_embeddings(graph, node_id, node_name, provider)
}

#[cfg(not(feature = "embeddings"))]
pub(crate) fn handle_review(graph: &AilGraph, input: ReviewInput) -> ReviewOutput {
    // 1. Resolve node.
    let node_id = match resolve_node_id(graph, &input.node) {
        Some(id) => id,
        None => {
            return unavailable_response(
                input.node.clone(),
                None,
                &format!("Node not found: \"{}\"", input.node),
            );
        }
    };

    let node_name = match graph.get_node(node_id) {
        Ok(n) => n.metadata.name.clone(),
        Err(_) => {
            return unavailable_response(
                node_id.to_string(),
                None,
                &format!("Node not found: \"{}\"", input.node),
            );
        }
    };

    unavailable_response(
        node_id.to_string(),
        node_name,
        "MCP server built without `embeddings` feature. Rebuild with `cargo build --features embeddings`.",
    )
}

// ── Embeddings path ───────────────────────────────────────────────────────────

/// Inner embeddings-gated implementation.
///
/// `cached_provider` is the pre-built `OnnxEmbeddingProvider` supplied by
/// `McpServer` (built at most once per session). When `None`, the provider is
/// unavailable and the function returns an `"Unavailable"` response directly
/// — no re-init attempt is made, avoiding the expensive `ensure_model` call
/// on every request when model files are simply absent.
#[cfg(feature = "embeddings")]
fn handle_review_with_embeddings(
    graph: &AilGraph,
    node_id: NodeId,
    node_name: Option<String>,
    cached_provider: Option<&ail_search::OnnxEmbeddingProvider>,
) -> ReviewOutput {
    use ail_coverage::compute_coverage;
    use ail_graph::cic::CoverageConfig;

    let node_id_str = node_id.to_string();

    // Use the pre-built provider from the server-level cache.
    let provider = match cached_provider {
        Some(p) => p,
        None => {
            return unavailable_response(
                node_id_str,
                node_name,
                "Embeddings not available. Run `ail reindex --embeddings` first.",
            );
        }
    };

    let cfg = CoverageConfig::default();
    let extra_concepts: Vec<String> = vec![];

    let result = match compute_coverage(graph, provider, node_id, &extra_concepts) {
        Ok(r) => r,
        Err(e) => {
            return unavailable_response(
                node_id_str,
                node_name,
                &format!(
                    "Embeddings not available. Run `ail reindex --embeddings` first. ({})",
                    e
                ),
            );
        }
    };

    // Convert to ReviewOutput.
    let config_hash = cfg.config_hash();
    let info = result.into_info(&cfg, config_hash);

    // Leaf node (Guard D).
    if info.score.is_none() {
        let suggestion = build_suggestion("N/A", &[]);
        return ReviewOutput {
            node_id: node_id_str,
            node_name,
            coverage: None,
            status: "N/A".to_owned(),
            children_coverage: vec![],
            missing: vec![],
            suggestion,
            empty_parent: None,
            degenerate_basis_fallback: None,
        };
    }

    let status = info.status.label().to_owned();
    let missing: Vec<MissingItem> = info
        .missing_aspects
        .iter()
        .map(|m| MissingItem {
            concept: m.concept.clone(),
            similarity: m.similarity,
        })
        .collect();
    let suggestion = build_suggestion(&status, &missing);

    let children_coverage: Vec<ChildCoverageItem> = info
        .child_contributions
        .into_iter()
        .map(|c| ChildCoverageItem {
            node_id: c.node_id.clone(),
            intent_preview: c.intent_preview,
            contribution: c.projection_magnitude,
        })
        .collect();

    ReviewOutput {
        node_id: node_id_str,
        node_name,
        coverage: info.score,
        status,
        children_coverage,
        missing,
        suggestion,
        empty_parent: if info.empty_parent { Some(true) } else { None },
        degenerate_basis_fallback: if info.degenerate_basis_fallback {
            Some(true)
        } else {
            None
        },
    }
}
