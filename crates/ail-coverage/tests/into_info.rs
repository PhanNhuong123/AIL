//! Task 13.2 Phase A+B — integration tests for `CoverageResult::into_info`.
//!
//! These tests exercise pure data-type mapping from `CoverageResult` (the raw
//! algorithm output) into `CoverageInfo` (the serializable packet attachment).
//! No mock providers or graph traversal are required.

use ail_coverage::{
    ChildContribution, CoverageConfig, CoverageResult, CoverageStatus, MissingAspect,
};
use ail_graph::NodeId;

// ─── helpers ──────────────────────────────────────────────────────────────────

/// Build a minimal `CoverageResult` with `score: None` and no children.
fn leaf_result() -> CoverageResult {
    CoverageResult {
        score: None,
        child_contributions: vec![],
        missing_aspects: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
    }
}

/// Build a `CoverageResult` with a non-trivial score and one child.
fn scored_result(score: f32, empty_parent: bool) -> CoverageResult {
    let child_id = NodeId::new();
    CoverageResult {
        score: Some(score),
        child_contributions: vec![ChildContribution {
            node_id: child_id,
            intent_preview: "handle payment".to_owned(),
            projection_magnitude: 0.8,
        }],
        missing_aspects: vec![MissingAspect {
            concept: "authentication".to_owned(),
            similarity: 0.35,
        }],
        empty_parent,
        degenerate_basis_fallback: false,
    }
}

// ─── t132_into_info_maps_leaf_score_to_status_leaf ────────────────────────────

#[test]
fn t132_into_info_maps_leaf_score_to_status_leaf() {
    let result = leaf_result();
    let cfg = CoverageConfig::default();
    let hash = cfg.config_hash();

    let info = result.into_info(&cfg, hash);

    assert_eq!(
        info.status,
        CoverageStatus::Leaf,
        "score: None must map to status Leaf"
    );
    assert!(info.score.is_none());
    assert!(info.child_contributions.is_empty());
    assert!(info.missing_aspects.is_empty());
    assert!(!info.empty_parent);
    assert!(!info.degenerate_basis_fallback);
}

// ─── t132_into_info_preserves_empty_parent_flag ───────────────────────────────

#[test]
fn t132_into_info_preserves_empty_parent_flag() {
    // Guard A scenario: empty_parent = true, score = Some(0.0)
    let result = scored_result(0.0, true);
    let cfg = CoverageConfig::default();
    let hash = cfg.config_hash();

    let info = result.into_info(&cfg, hash);

    assert!(
        info.empty_parent,
        "empty_parent flag must survive into_info conversion"
    );
    assert_eq!(
        info.status,
        CoverageStatus::Weak,
        "score 0.0 is below both thresholds → Weak"
    );
}

// ─── t132_into_info_config_hash_attached ──────────────────────────────────────

#[test]
fn t132_into_info_config_hash_attached() {
    let result = scored_result(0.95, false);
    let cfg = CoverageConfig::default();
    let expected_hash = cfg.config_hash();

    let info = result.into_info(&cfg, expected_hash.clone());

    assert_eq!(
        info.config_hash, expected_hash,
        "config hash must be stored verbatim in CoverageInfo"
    );

    // Also verify child_contributions and missing_aspects were mapped.
    assert_eq!(info.child_contributions.len(), 1);
    assert_eq!(info.child_contributions[0].intent_preview, "handle payment");
    assert_eq!(info.missing_aspects.len(), 1);
    assert_eq!(info.missing_aspects[0].concept, "authentication");

    // Status from score 0.95 ≥ 0.9 threshold_full
    assert_eq!(info.status, CoverageStatus::Full);
}
