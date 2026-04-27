//! Unit tests for Phase 17.4 sheaf analysis pure helpers and DTOs.
//!
//! Covers `seed_sheaf_nonce`, `next_sheaf_run_id_string`, `SheafCompletePayload`
//! serde round-trip, `SheafCancelResult` serde, `SheafConflictEntry` serde, and
//! `BridgeStateInner` field initialization for the 4 new sheaf fields.

#![cfg(feature = "tauri-commands")]

use std::sync::atomic::Ordering;

use ail_ui_bridge::sheaf::{next_sheaf_run_id_string, seed_sheaf_nonce};
use ail_ui_bridge::types::sheaf::{SheafCancelResult, SheafCompletePayload, SheafConflictEntry};
use ail_ui_bridge::BridgeStateInner;

// ---------------------------------------------------------------------------
// SH1 — seed_sheaf_nonce is non-zero
// ---------------------------------------------------------------------------

#[test]
fn test_seed_sheaf_nonce_nonzero() {
    let n = seed_sheaf_nonce();
    assert_ne!(n, 0, "nonce must be non-zero on a normal system");
}

// ---------------------------------------------------------------------------
// SH2 — next_sheaf_run_id_string is monotonic with "sheaf-" prefix
// ---------------------------------------------------------------------------

#[test]
fn test_next_sheaf_run_id_string_is_monotonic_with_prefix() {
    let nonce: u64 = 0xCAFE_BABE_1234_5678;
    let mut seq: u64 = 0;

    let a = next_sheaf_run_id_string(&mut seq, nonce);
    let b = next_sheaf_run_id_string(&mut seq, nonce);
    let c = next_sheaf_run_id_string(&mut seq, nonce);

    // All ids must start with the "sheaf-" prefix.
    assert!(
        a.starts_with("sheaf-"),
        "run id must have 'sheaf-' prefix: {a}"
    );
    assert!(
        b.starts_with("sheaf-"),
        "run id must have 'sheaf-' prefix: {b}"
    );
    assert!(
        c.starts_with("sheaf-"),
        "run id must have 'sheaf-' prefix: {c}"
    );

    // Each call must produce a distinct string.
    assert_ne!(a, b, "run ids must be distinct");
    assert_ne!(b, c, "run ids must be distinct");
    assert_ne!(a, c, "run ids must be distinct");

    // Sequence counter must have incremented three times.
    assert_eq!(seq, 3, "seq must advance by 1 per call");
}

// ---------------------------------------------------------------------------
// SH3 — SheafCompletePayload serde round-trip with camelCase checks
// ---------------------------------------------------------------------------

#[test]
fn test_sheaf_complete_payload_serde_roundtrip_camelcase() {
    let entry = SheafConflictEntry {
        overlap_index: 0,
        node_a: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string(),
        node_b: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".to_string(),
        conflicting_a: vec!["amount > 10".to_string()],
        conflicting_b: vec!["amount < 5".to_string()],
    };

    let payload = SheafCompletePayload {
        run_id: "sheaf-1-abc".to_string(),
        ok: true,
        z3_available: true,
        conflicts: vec![entry.clone()],
        cancelled: false,
        error: None,
    };

    let json = serde_json::to_string(&payload).unwrap();

    // Wire field names must be camelCase.
    assert!(
        json.contains("\"runId\":\"sheaf-1-abc\""),
        "runId must be camelCase: {json}"
    );
    assert!(
        json.contains("\"z3Available\":true"),
        "z3Available must be camelCase: {json}"
    );
    assert!(
        json.contains("\"overlapIndex\":0"),
        "overlapIndex must be camelCase: {json}"
    );
    assert!(
        json.contains("\"nodeA\":"),
        "nodeA must be camelCase: {json}"
    );
    assert!(
        json.contains("\"nodeB\":"),
        "nodeB must be camelCase: {json}"
    );
    assert!(
        json.contains("\"conflictingA\":"),
        "conflictingA must be camelCase: {json}"
    );
    assert!(
        json.contains("\"conflictingB\":"),
        "conflictingB must be camelCase: {json}"
    );

    // `cancelled: false` must be omitted (skip_serializing_if = "is_false").
    assert!(
        !json.contains("\"cancelled\""),
        "cancelled=false must be omitted from JSON: {json}"
    );

    // `error: None` must be omitted (skip_serializing_if = "Option::is_none").
    assert!(
        !json.contains("\"error\""),
        "error=None must be omitted from JSON: {json}"
    );

    // Round-trip must produce an equal value.
    let back: SheafCompletePayload = serde_json::from_str(&json).unwrap();
    assert_eq!(back, payload, "payload must survive serde roundtrip");
    assert_eq!(back.conflicts.len(), 1);
    assert_eq!(back.conflicts[0], entry);
}

// ---------------------------------------------------------------------------
// SH4 — SheafCancelResult serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_sheaf_cancel_result_serde() {
    let result = SheafCancelResult { cancelled: true };
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(json, r#"{"cancelled":true}"#, "wire shape must match");
    let back: SheafCancelResult = serde_json::from_str(&json).unwrap();
    assert!(back.cancelled, "cancelled must survive roundtrip");
}

// ---------------------------------------------------------------------------
// SH5 — SheafConflictEntry serde round-trip with non-empty constraint vectors
// ---------------------------------------------------------------------------

#[test]
fn test_sheaf_conflict_entry_serde_roundtrip() {
    let entry = SheafConflictEntry {
        overlap_index: 3,
        node_a: "11111111-1111-1111-1111-111111111111".to_string(),
        node_b: "22222222-2222-2222-2222-222222222222".to_string(),
        conflicting_a: vec![
            "amount > 100".to_string(),
            "status == \"active\"".to_string(),
        ],
        conflicting_b: vec!["amount < 50".to_string()],
    };

    let json = serde_json::to_string(&entry).unwrap();
    let back: SheafConflictEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(
        back, entry,
        "SheafConflictEntry must survive serde roundtrip"
    );
    assert_eq!(back.overlap_index, 3);
    assert_eq!(back.conflicting_a.len(), 2);
    assert_eq!(back.conflicting_b.len(), 1);
    assert_eq!(back.node_a, "11111111-1111-1111-1111-111111111111");
    assert_eq!(back.node_b, "22222222-2222-2222-2222-222222222222");
}

// ---------------------------------------------------------------------------
// SH6 — BridgeStateInner struct literal compiles and has correct sheaf defaults
// ---------------------------------------------------------------------------

#[test]
fn test_fresh_inner_has_sheaf_fields() {
    let inner = BridgeStateInner {
        project_path: None,
        graph_json: None,
        watcher: None,
        load_generation: 0,
        agent_run: None,
        agent_run_seq: 0,
        agent_id_nonce: 0,
        verifier_run: None,
        verifier_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        verifier_run_seq: 0,
        verifier_id_nonce: 0,
        sheaf_run: None,
        sheaf_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        sheaf_run_seq: 0,
        sheaf_id_nonce: 42,
        sidecar_health_seq: 0,
        sidecar_id_nonce: 0,
    };

    assert!(inner.sheaf_run.is_none(), "sheaf_run must start as None");
    assert_eq!(inner.sheaf_run_seq, 0, "sheaf_run_seq must start at 0");
    assert!(
        !inner.sheaf_cancelled.load(Ordering::SeqCst),
        "sheaf_cancelled fence must start false"
    );
    assert_eq!(
        inner.sheaf_id_nonce, 42,
        "sheaf_id_nonce must match what was set"
    );
}

// ---------------------------------------------------------------------------
// SH7 / SH7-bis — project_to_sheaf_conflicts translates NodeId UUIDs to path-like IDs
// ---------------------------------------------------------------------------

#[test]
#[cfg(feature = "z3-verify")]
fn test_project_to_sheaf_conflicts_translates_uuids_to_path_ids() {
    use ail_contract::{ObstructionResult, ObstructionStatus};
    use ail_graph::NodeId;
    use ail_ui_bridge::ids::IdMap;
    use ail_ui_bridge::sheaf::project_to_sheaf_conflicts;
    use std::collections::BTreeMap;

    // Two distinct node IDs.
    let id_a = NodeId::default();
    let id_b = NodeId::default();
    let path_a = "module.fn.step_a".to_string();
    let path_b = "module.fn.step_b".to_string();

    // Build a hand-crafted IdMap without needing a real graph.
    let mut forward = BTreeMap::new();
    forward.insert(id_a.to_string(), path_a.clone());
    forward.insert(id_b.to_string(), path_b.clone());
    let mut reverse = BTreeMap::new();
    reverse.insert(path_a.clone(), id_a);
    reverse.insert(path_b.clone(), id_b);
    let id_map = IdMap { forward, reverse };

    let obstructions = vec![ObstructionResult {
        overlap_index: 0,
        node_a: id_a,
        node_b: id_b,
        status: ObstructionStatus::Contradictory {
            conflicting_a: vec![],
            conflicting_b: vec![],
        },
    }];

    let entries = project_to_sheaf_conflicts(&obstructions, &id_map);

    assert_eq!(
        entries.len(),
        1,
        "one Contradictory entry must be projected"
    );
    assert_eq!(
        entries[0].node_a, "module.fn.step_a",
        "node_a must be the path-like ID, not the UUID"
    );
    assert_eq!(
        entries[0].node_b, "module.fn.step_b",
        "node_b must be the path-like ID, not the UUID"
    );
    assert_eq!(entries[0].overlap_index, 0);
}

#[test]
#[cfg(feature = "z3-verify")]
fn test_project_to_sheaf_conflicts_skips_entry_with_missing_path() {
    use ail_contract::{ObstructionResult, ObstructionStatus};
    use ail_graph::NodeId;
    use ail_ui_bridge::ids::IdMap;
    use ail_ui_bridge::sheaf::project_to_sheaf_conflicts;
    use std::collections::BTreeMap;

    // id_a is in the map, id_b is NOT — defensive skip.
    let id_a = NodeId::default();
    let id_b = NodeId::default();
    let path_a = "module.fn.step_a".to_string();

    let mut forward = BTreeMap::new();
    forward.insert(id_a.to_string(), path_a.clone());
    let mut reverse = BTreeMap::new();
    reverse.insert(path_a.clone(), id_a);
    let id_map = IdMap { forward, reverse };

    let obstructions = vec![ObstructionResult {
        overlap_index: 1,
        node_a: id_a,
        node_b: id_b, // not in map
        status: ObstructionStatus::Contradictory {
            conflicting_a: vec![],
            conflicting_b: vec![],
        },
    }];

    let entries = project_to_sheaf_conflicts(&obstructions, &id_map);
    assert_eq!(
        entries.len(),
        0,
        "entry with unmapped node_b must be skipped defensively"
    );
}
