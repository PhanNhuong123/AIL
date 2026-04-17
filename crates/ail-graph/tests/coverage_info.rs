//! Integration tests for CIC coverage types (task 13.2 Phase A+B).
//!
//! Test prefix: `t132_`

use ail_graph::{ContextPacket, CoverageConfig, CoverageStatus, NodeId};

// ─── t132_coverage_status_from_score_thresholds ───────────────────────────────

#[test]
fn t132_coverage_status_from_score_thresholds() {
    let cfg = CoverageConfig::default(); // full=0.9, partial=0.7

    // Leaf: None score
    assert_eq!(CoverageStatus::from_score(None, &cfg), CoverageStatus::Leaf);

    // Full: at and above threshold_full
    assert_eq!(
        CoverageStatus::from_score(Some(0.9), &cfg),
        CoverageStatus::Full
    );
    assert_eq!(
        CoverageStatus::from_score(Some(1.0), &cfg),
        CoverageStatus::Full
    );

    // Partial: at threshold_partial, below threshold_full
    assert_eq!(
        CoverageStatus::from_score(Some(0.7), &cfg),
        CoverageStatus::Partial
    );
    assert_eq!(
        CoverageStatus::from_score(Some(0.89), &cfg),
        CoverageStatus::Partial
    );

    // Weak: below threshold_partial
    assert_eq!(
        CoverageStatus::from_score(Some(0.0), &cfg),
        CoverageStatus::Weak
    );
    assert_eq!(
        CoverageStatus::from_score(Some(0.69), &cfg),
        CoverageStatus::Weak
    );
}

// ─── t132_coverage_status_label_maps_leaf_to_na ───────────────────────────────

#[test]
fn t132_coverage_status_label_maps_leaf_to_na() {
    assert_eq!(CoverageStatus::Leaf.label(), "N/A");

    // Serialization must produce "N/A" in JSON
    let json = serde_json::to_string(&CoverageStatus::Leaf).expect("serialize Leaf");
    assert_eq!(json, "\"N/A\"");

    // Deserialize back
    let back: CoverageStatus = serde_json::from_str(&json).expect("deserialize N/A");
    assert_eq!(back, CoverageStatus::Leaf);
}

// ─── t132_coverage_config_hash_stable ────────────────────────────────────────

#[test]
fn t132_coverage_config_hash_stable() {
    let cfg = CoverageConfig::default();

    // Same config → same hash
    assert_eq!(cfg.config_hash(), cfg.config_hash());

    // Changed threshold → different hash
    let cfg_modified = CoverageConfig {
        threshold_full: 0.85,
        ..CoverageConfig::default()
    };
    assert_ne!(cfg.config_hash(), cfg_modified.config_hash());

    // extra_concepts order-independent
    let cfg_a = CoverageConfig {
        extra_concepts: vec!["alpha".to_owned(), "beta".to_owned()],
        ..CoverageConfig::default()
    };
    let cfg_b = CoverageConfig {
        extra_concepts: vec!["beta".to_owned(), "alpha".to_owned()],
        ..CoverageConfig::default()
    };
    assert_eq!(
        cfg_a.config_hash(),
        cfg_b.config_hash(),
        "concept list order must not affect hash"
    );
}

// ─── t132_context_packet_deserializes_missing_coverage_field ─────────────────

#[test]
fn t132_context_packet_deserializes_missing_coverage_field() {
    // Build a minimal valid ContextPacket JSON without a "coverage" key.
    let node_id = NodeId::new();
    let json = serde_json::json!({
        "node_id": node_id,
        "intent_chain": [],
        "inherited_constraints": [],
        "type_constraints": [],
        "call_contracts": [],
        "template_constraints": [],
        "verified_facts": [],
        "promoted_facts": [],
        "scope": [],
        "must_produce": null
        // NOTE: no "coverage" key
    });

    let packet: ContextPacket =
        serde_json::from_value(json).expect("should deserialize without coverage key");

    assert!(
        packet.coverage.is_none(),
        "coverage must default to None when absent from JSON"
    );
}
