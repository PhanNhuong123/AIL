//! Unit tests for the Phase 16.3 verifier pure helpers.
//!
//! Covers `seed_verifier_nonce`, `next_verifier_run_id_string`,
//! `collect_scope_ids`, and `VerifyCompletePayload` serde round-trip.
//! The full async Tauri commands (`run_verifier`, `cancel_verifier_run`)
//! require a live `AppHandle` and are covered by integration tests in `ide/`.

#![cfg(feature = "tauri-commands")]

use std::collections::BTreeMap;

use ail_ui_bridge::types::graph_json::{
    FunctionJson, GraphJson, ModuleJson, ProjectJson, StepJson,
};
use ail_ui_bridge::types::status::Status;
use ail_ui_bridge::types::verify_result::{VerifyCompletePayload, VerifyFailureJson};
use ail_ui_bridge::verifier::{
    collect_scope_ids, next_verifier_run_id_string, seed_verifier_nonce,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_step(id: &str) -> StepJson {
    StepJson {
        id: id.to_string(),
        name: id.to_string(),
        status: Status::Ok,
        intent: "do".to_string(),
        branch: None,
    }
}

fn make_function(id: &str, steps: Vec<StepJson>) -> FunctionJson {
    FunctionJson {
        id: id.to_string(),
        name: id.to_string(),
        status: Status::Ok,
        steps: if steps.is_empty() { None } else { Some(steps) },
    }
}

fn make_module(id: &str, functions: Vec<FunctionJson>) -> ModuleJson {
    ModuleJson {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        cluster: String::new(),
        cluster_name: String::new(),
        cluster_color: String::new(),
        status: Status::Ok,
        node_count: functions.len(),
        functions,
    }
}

fn make_graph_with_one_module() -> GraphJson {
    let step_a = make_step("mod1/fn1/step1");
    let step_b = make_step("mod1/fn1/step2");
    let func1 = make_function("mod1/fn1", vec![step_a, step_b]);
    let func2 = make_function("mod1/fn2", vec![]);
    let module = make_module("mod1", vec![func1, func2]);

    GraphJson {
        project: ProjectJson {
            id: "proj".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: 5,
            module_count: 1,
            fn_count: 2,
            status: Status::Ok,
        },
        clusters: vec![],
        modules: vec![module],
        externals: vec![],
        relations: vec![],
        types: vec![],
        errors: vec![],
        issues: vec![],
        detail: BTreeMap::new(),
    }
}

fn make_graph_two_modules() -> GraphJson {
    let step = make_step("m2/fn3/s1");
    let fn3 = make_function("m2/fn3", vec![step]);
    let mod2 = make_module("m2", vec![fn3]);

    let fn1 = make_function("m1/fn1", vec![]);
    let mod1 = make_module("m1", vec![fn1]);

    GraphJson {
        project: ProjectJson {
            id: "proj2".to_string(),
            name: "test2".to_string(),
            description: String::new(),
            node_count: 4,
            module_count: 2,
            fn_count: 2,
            status: Status::Ok,
        },
        clusters: vec![],
        modules: vec![mod1, mod2],
        externals: vec![],
        relations: vec![],
        types: vec![],
        errors: vec![],
        issues: vec![],
        detail: BTreeMap::new(),
    }
}

// ---------------------------------------------------------------------------
// V1 — next_verifier_run_id_string is monotonic
// ---------------------------------------------------------------------------

#[test]
fn test_next_verifier_run_id_string_is_monotonic() {
    let nonce: u64 = 0xABCD_1234;
    let mut seq: u64 = 0;

    let a = next_verifier_run_id_string(&mut seq, nonce);
    let b = next_verifier_run_id_string(&mut seq, nonce);
    let c = next_verifier_run_id_string(&mut seq, nonce);

    // Each call must produce a distinct string.
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);

    // All ids must start with the "verify-" prefix.
    assert!(
        a.starts_with("verify-"),
        "run id must have 'verify-' prefix: {a}"
    );
    assert!(
        b.starts_with("verify-"),
        "run id must have 'verify-' prefix: {b}"
    );
    assert!(
        c.starts_with("verify-"),
        "run id must have 'verify-' prefix: {c}"
    );

    // Sequence counter must have incremented three times.
    assert_eq!(seq, 3);
}

// ---------------------------------------------------------------------------
// V2 — seed_verifier_nonce is non-zero
// ---------------------------------------------------------------------------

#[test]
fn test_seed_verifier_nonce_nonzero() {
    let n = seed_verifier_nonce();
    assert_ne!(n, 0, "nonce must be non-zero on a normal system");
}

// ---------------------------------------------------------------------------
// V3 — collect_scope_ids: "project" returns all node ids
// ---------------------------------------------------------------------------

#[test]
fn test_collect_scope_ids_project_returns_all_node_ids() {
    let graph = make_graph_with_one_module();
    let ids = collect_scope_ids(&graph, "project", None);

    // Expected: mod1, mod1/fn1, mod1/fn1/step1, mod1/fn1/step2, mod1/fn2
    assert!(ids.contains(&"mod1".to_string()));
    assert!(ids.contains(&"mod1/fn1".to_string()));
    assert!(ids.contains(&"mod1/fn1/step1".to_string()));
    assert!(ids.contains(&"mod1/fn1/step2".to_string()));
    assert!(ids.contains(&"mod1/fn2".to_string()));
    assert_eq!(ids.len(), 5);
}

// ---------------------------------------------------------------------------
// V4 — collect_scope_ids: "module" returns module + fns + steps
// ---------------------------------------------------------------------------

#[test]
fn test_collect_scope_ids_module_returns_module_and_descendants() {
    let graph = make_graph_two_modules();

    // Scope to "m2" only.
    let ids = collect_scope_ids(&graph, "module", Some("m2"));

    assert!(
        ids.contains(&"m2".to_string()),
        "must include module itself"
    );
    assert!(ids.contains(&"m2/fn3".to_string()), "must include function");
    assert!(ids.contains(&"m2/fn3/s1".to_string()), "must include step");
    // m1 and its function must NOT be included.
    assert!(
        !ids.contains(&"m1".to_string()),
        "must not include other modules"
    );
    assert!(
        !ids.contains(&"m1/fn1".to_string()),
        "must not include other functions"
    );
    assert_eq!(ids.len(), 3);
}

// ---------------------------------------------------------------------------
// V4b — collect_scope_ids: unknown scope_id returns empty vec (C2 contract)
// ---------------------------------------------------------------------------

#[test]
fn test_collect_scope_ids_unknown_id_returns_empty() {
    let graph = make_graph_with_one_module();

    // Unknown module id → empty (not all-ids fallback)
    let ids = collect_scope_ids(&graph, "module", Some("no-such-module"));
    assert!(
        ids.is_empty(),
        "unknown module id must return empty: {ids:?}"
    );

    // None scope_id for module → empty
    let ids2 = collect_scope_ids(&graph, "module", None);
    assert!(
        ids2.is_empty(),
        "None module scope_id must return empty: {ids2:?}"
    );

    // Unknown function id → empty
    let ids3 = collect_scope_ids(&graph, "function", Some("no-such-fn"));
    assert!(
        ids3.is_empty(),
        "unknown function id must return empty: {ids3:?}"
    );

    // Unknown step id → empty
    let ids4 = collect_scope_ids(&graph, "step", Some("no-such-step"));
    assert!(
        ids4.is_empty(),
        "unknown step id must return empty: {ids4:?}"
    );

    // Unknown scope value → empty
    let ids5 = collect_scope_ids(&graph, "unknown_scope", None);
    assert!(
        ids5.is_empty(),
        "unknown scope value must return empty: {ids5:?}"
    );
}

// ---------------------------------------------------------------------------
// V5 — VerifyCompletePayload serde round-trip (camelCase + skip falsy fields)
// ---------------------------------------------------------------------------

#[test]
fn test_verify_complete_payload_serde_roundtrip_camelcase() {
    let payload = VerifyCompletePayload {
        ok: true,
        failures: vec![],
        run_id: "verify-1-abc".to_string(),
        scope: "project".to_string(),
        scope_id: None,
        node_ids: vec![],
        cancelled: false,
    };

    let json = serde_json::to_string(&payload).unwrap();

    // Wire field names must be camelCase.
    assert!(
        json.contains("\"runId\":\"verify-1-abc\""),
        "runId field must be camelCase: {json}"
    );
    assert!(
        json.contains("\"nodeIds\":[]"),
        "nodeIds field must be camelCase: {json}"
    );

    // `cancelled: false` must be skipped (skip_serializing_if = "is_false").
    assert!(
        !json.contains("\"cancelled\""),
        "cancelled=false must be omitted: {json}"
    );

    // `scope_id: None` must be skipped.
    assert!(
        !json.contains("\"scopeId\""),
        "scopeId=None must be omitted: {json}"
    );

    // Round-trip must produce an equal value.
    let back: VerifyCompletePayload = serde_json::from_str(&json).unwrap();
    assert_eq!(back, payload);

    // When cancelled=true it MUST be present in the serialized output.
    let cancelled_payload = VerifyCompletePayload {
        ok: true,
        failures: vec![],
        run_id: "verify-2-xyz".to_string(),
        scope: "cancelled".to_string(),
        scope_id: None,
        node_ids: vec![],
        cancelled: true,
    };
    let cancelled_json = serde_json::to_string(&cancelled_payload).unwrap();
    assert!(
        cancelled_json.contains("\"cancelled\":true"),
        "cancelled=true must be present: {cancelled_json}"
    );
}

// ---------------------------------------------------------------------------
// V6 — VerifyFailureJson outcome field round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_verify_failure_json_outcome_field_roundtrip() {
    // outcome: None → must be omitted from JSON.
    let f = VerifyFailureJson {
        node_id: "mod/fn".to_string(),
        message: "pre-condition violated".to_string(),
        stage: None,
        severity: Some("fail".to_string()),
        source: Some("verify".to_string()),
        outcome: None,
    };
    let json = serde_json::to_string(&f).unwrap();
    assert!(
        !json.contains("outcome"),
        "outcome=None must be omitted: {json}"
    );

    // outcome: Some("timeout") → must round-trip.
    let f2 = VerifyFailureJson {
        outcome: Some("timeout".to_string()),
        ..f.clone()
    };
    let json2 = serde_json::to_string(&f2).unwrap();
    assert!(
        json2.contains("\"outcome\":\"timeout\""),
        "outcome must be present when Some: {json2}"
    );
    let back: VerifyFailureJson = serde_json::from_str(&json2).unwrap();
    assert_eq!(back, f2);
}
