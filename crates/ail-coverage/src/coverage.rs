use ail_graph::{GraphBackend, NodeId};
use ail_search::{node_embedding_text, EmbeddingProvider};

use crate::concepts::combined_concepts;
use crate::errors::CoverageError;
use crate::missing::detect_missing_aspects;
use crate::projection::{
    average_cosine_fallback, build_orthonormal_basis, clamp_score_with_log, l2_norm,
    normalize_in_place, project_onto_basis, PARENT_NORM_EPSILON,
};
use crate::types::{truncate_preview, ChildContribution, CoverageResult};

/// Compute semantic coverage for the node identified by `node_id`.
///
/// Uses the v2.0 canonical text form (`node_embedding_text`) for both parent
/// and children so coverage vectors are consistent with the v2.0 search index.
///
/// # Parameters
///
/// - `graph` — any `GraphBackend` implementation (in-memory or SQLite).
/// - `provider` — embedding provider; must agree on dimensions for all texts.
/// - `node_id` — the parent node to evaluate.
/// - `extra_concepts` — caller-supplied concept labels that extend the built-in
///   40-entry list for missing-aspect detection.  Blank entries are ignored.
///
/// # Return value
///
/// - `score: None` — `node_id` has no children (Guard D; leaf N/A).
/// - `score: Some(0.0)` + `empty_parent: true` — parent embedding near-zero (Guard A).
/// - `score: Some(x)` — normal bounded score `x ∈ [0.0, 1.0]`.
pub fn compute_coverage(
    graph: &dyn GraphBackend,
    provider: &dyn EmbeddingProvider,
    node_id: NodeId,
    extra_concepts: &[String],
) -> Result<CoverageResult, CoverageError> {
    // ── 1. Fetch parent node ──────────────────────────────────────────────────
    let parent = graph
        .get_node(node_id)?
        .ok_or(CoverageError::NodeNotFound(node_id))?;

    // ── 2. Enumerate non-blank children ──────────────────────────────────────
    let child_ids = graph.children(node_id)?;
    let mut children = Vec::new();
    for id in child_ids {
        if let Some(node) = graph.get_node(id)? {
            if !node.intent.trim().is_empty() {
                children.push(node);
            }
        }
    }

    // ── 3. Guard D — leaf node ───────────────────────────────────────────────
    if children.is_empty() {
        return Ok(CoverageResult {
            score: None,
            child_contributions: vec![],
            missing_aspects: vec![],
            empty_parent: false,
            degenerate_basis_fallback: false,
        });
    }

    // ── 4. Validate provider dimension ───────────────────────────────────────
    let dim = provider.dimension();
    if dim == 0 {
        return Err(CoverageError::ZeroDimension);
    }

    // ── 5. Build batch text list: [parent, child1, ..., childN] ─────────────
    let parent_text = node_embedding_text(&parent);
    let child_texts: Vec<String> = children.iter().map(node_embedding_text).collect();

    let mut all_texts: Vec<String> = Vec::with_capacity(1 + children.len());
    all_texts.push(parent_text);
    all_texts.extend(child_texts);

    let text_refs: Vec<&str> = all_texts.iter().map(String::as_str).collect();

    // ── 6. Embed entire batch ────────────────────────────────────────────────
    let vectors = provider.embed_batch(&text_refs)?;

    // ── 7. Validate all vector lengths ───────────────────────────────────────
    for v in &vectors {
        if v.len() != dim {
            return Err(CoverageError::DimensionMismatch {
                expected: dim,
                actual: v.len(),
            });
        }
    }

    // ── 8. Split parent + children ───────────────────────────────────────────
    let parent_vec = vectors[0].clone();
    let children_vecs: Vec<Vec<f32>> = vectors[1..].to_vec();

    // ── 9. Guard A — near-zero parent ────────────────────────────────────────
    let parent_norm = l2_norm(&parent_vec);
    if parent_norm < PARENT_NORM_EPSILON {
        log::warn!(
            "ail-coverage: parent intent vector near-zero (norm={parent_norm:.2e}); reporting empty_parent"
        );
        let contributions = children
            .iter()
            .map(|c| ChildContribution {
                node_id: c.id,
                intent_preview: truncate_preview(&c.intent),
                projection_magnitude: 0.0,
            })
            .collect();
        return Ok(CoverageResult {
            score: Some(0.0),
            child_contributions: contributions,
            missing_aspects: vec![],
            empty_parent: true,
            degenerate_basis_fallback: false,
        });
    }

    // ── 10. Normalize parent and all children ────────────────────────────────
    let mut parent_unit = parent_vec.clone();
    normalize_in_place(&mut parent_unit);

    let mut children_normed: Vec<Vec<f32>> = children_vecs.clone();
    for v in &mut children_normed {
        normalize_in_place(v);
    }

    // ── 11. Build orthonormal basis (Guard B inside) ─────────────────────────
    let basis_outcome = build_orthonormal_basis(&children_normed);

    // ── 12. Guard C — degenerate basis ───────────────────────────────────────
    if basis_outcome.basis.is_empty() {
        log::warn!("ail-coverage: orthonormal basis degenerated to empty set; using average-cosine fallback");
        let fallback_score = average_cosine_fallback(&parent_unit, &children_normed);
        let contributions = children
            .iter()
            .map(|c| ChildContribution {
                node_id: c.id,
                intent_preview: truncate_preview(&c.intent),
                projection_magnitude: 0.0,
            })
            .collect();
        return Ok(CoverageResult {
            score: Some(fallback_score),
            child_contributions: contributions,
            missing_aspects: vec![],
            empty_parent: false,
            degenerate_basis_fallback: true,
        });
    }

    // ── 13. Project parent onto basis ────────────────────────────────────────
    let proj = project_onto_basis(&parent_unit, &basis_outcome.basis);
    let residual_norm = l2_norm(&proj.residual);
    // Parent was L2-normalized so norm(parent_unit) == 1.0 and
    // score = 1.0 - norm(residual).
    let unclamped = 1.0 - residual_norm;
    let score = clamp_score_with_log(unclamped);

    // ── 14. Build child contributions ────────────────────────────────────────
    // `basis_outcome.accepted_child_indices[i]` maps basis vector i to child
    // index; `proj.per_basis_projection_magnitudes[i]` is the magnitude for
    // basis vector i.
    let mut child_magnitudes = vec![0.0f32; children.len()];
    for (basis_pos, &child_idx) in basis_outcome.accepted_child_indices.iter().enumerate() {
        child_magnitudes[child_idx] = proj.per_basis_projection_magnitudes[basis_pos];
    }

    let contributions: Vec<ChildContribution> = children
        .iter()
        .enumerate()
        .map(|(i, c)| ChildContribution {
            node_id: c.id,
            intent_preview: truncate_preview(&c.intent),
            projection_magnitude: child_magnitudes[i],
        })
        .collect();

    // ── 15. Missing-aspect detection ─────────────────────────────────────────
    let missing_aspects = if residual_norm < PARENT_NORM_EPSILON {
        // Residual is negligible — nothing missing.
        vec![]
    } else {
        let concepts = combined_concepts(extra_concepts);
        detect_missing_aspects(&proj.residual, &concepts, provider)?
    };

    Ok(CoverageResult {
        score: Some(score),
        child_contributions: contributions,
        missing_aspects,
        empty_parent: false,
        degenerate_basis_fallback: false,
    })
}
