use ail_ui_bridge::serialize::diff::diff_graph_at;
use ail_ui_bridge::types::patch::GraphPatchJson;
use ail_ui_bridge::types::status::Status;

#[path = "support/mod.rs"]
mod support;
use support::*;

// ---------------------------------------------------------------------------
// Module-level tests
// ---------------------------------------------------------------------------

/// Test 1: prev empty, next has 1 module → modules_added.len() == 1.
#[test]
fn test_module_added() {
    let prev = empty_graph();
    let next = graph_with_modules(vec![make_module("mod1", "Module 1", "desc", vec![])]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(patch.modules_added.len(), 1, "expected 1 added module");
    assert_eq!(patch.modules_added[0].id, "mod1");
    assert!(patch.modules_modified.is_empty(), "no modified");
    assert!(patch.modules_removed.is_empty(), "no removed");
    // All other arrays empty.
    assert_all_fn_and_step_arrays_empty(&patch);
}

/// Test 2: same id, different description → modules_modified.len() == 1.
#[test]
fn test_module_modified_on_description_change() {
    let prev = graph_with_modules(vec![make_module("mod1", "Module 1", "original", vec![])]);
    let next = graph_with_modules(vec![make_module("mod1", "Module 1", "changed", vec![])]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(
        patch.modules_modified.len(),
        1,
        "expected 1 modified module"
    );
    assert_eq!(patch.modules_modified[0].id, "mod1");
    assert!(patch.modules_added.is_empty(), "no added");
    assert!(patch.modules_removed.is_empty(), "no removed");
}

/// Test 3: module only in prev → modules_removed contains it.
#[test]
fn test_module_removed() {
    let prev = graph_with_modules(vec![make_module("mod1", "Module 1", "desc", vec![])]);
    let next = empty_graph();

    let patch = diff_graph_at(&prev, &next, 0);

    assert!(
        patch.modules_removed.contains(&"mod1".to_string()),
        "mod1 must be in removed"
    );
    assert!(patch.modules_added.is_empty(), "no added");
    assert!(patch.modules_modified.is_empty(), "no modified");
}

// ---------------------------------------------------------------------------
// Function-level tests
// ---------------------------------------------------------------------------

/// Test 4: added function carries module_id.
#[test]
fn test_function_added_with_module_id() {
    let prev = graph_with_modules(vec![make_module("mod1", "m1", "", vec![])]);
    let next = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Ok, None)],
    )]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(patch.functions_added.len(), 1, "expected 1 added function");
    assert_eq!(patch.functions_added[0].module_id, "mod1");
    assert_eq!(patch.functions_added[0].function.id, "mod1.fn1");
}

/// Test 5: same function id, different status → functions_modified.len() == 1.
#[test]
fn test_function_modified_status_change() {
    let prev = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Ok, None)],
    )]);
    let next = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Fail, None)],
    )]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(
        patch.functions_modified.len(),
        1,
        "expected 1 modified function"
    );
    assert_eq!(patch.functions_modified[0].function.status, Status::Fail);
    assert!(patch.functions_added.is_empty(), "no added");
    assert!(patch.functions_removed.is_empty(), "no removed");
}

/// Test 6: removed function carries module_id and function_id.
#[test]
fn test_function_removed_with_module_id() {
    let prev = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Ok, None)],
    )]);
    let next = graph_with_modules(vec![make_module("mod1", "m1", "", vec![])]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(
        patch.functions_removed.len(),
        1,
        "expected 1 removed function"
    );
    assert_eq!(patch.functions_removed[0].module_id, "mod1");
    assert_eq!(patch.functions_removed[0].function_id, "mod1.fn1");
}

// ---------------------------------------------------------------------------
// Step-level tests
// ---------------------------------------------------------------------------

/// Test 7: added step carries function_id.
#[test]
fn test_step_added_with_function_id() {
    let prev = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Ok, Some(vec![]))],
    )]);
    let next = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn(
            "mod1.fn1",
            "fn1",
            Status::Ok,
            Some(vec![make_step("mod1.fn1.s1", "s1", "validate")]),
        )],
    )]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(patch.steps_added.len(), 1, "expected 1 added step");
    assert_eq!(patch.steps_added[0].function_id, "mod1.fn1");
    assert_eq!(patch.steps_added[0].step.id, "mod1.fn1.s1");
}

/// Test 8: same step id, different intent → steps_modified.len() == 1.
#[test]
fn test_step_modified_intent_change() {
    let prev = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn(
            "mod1.fn1",
            "fn1",
            Status::Ok,
            Some(vec![make_step("mod1.fn1.s1", "s1", "original intent")]),
        )],
    )]);
    let next = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn(
            "mod1.fn1",
            "fn1",
            Status::Ok,
            Some(vec![make_step("mod1.fn1.s1", "s1", "changed intent")]),
        )],
    )]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(patch.steps_modified.len(), 1, "expected 1 modified step");
    assert_eq!(patch.steps_modified[0].step.intent, "changed intent");
    assert!(patch.steps_added.is_empty(), "no added");
    assert!(patch.steps_removed.is_empty(), "no removed");
}

/// Test 9: removed step carries function_id and step_id.
#[test]
fn test_step_removed_with_function_id() {
    let prev = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn(
            "mod1.fn1",
            "fn1",
            Status::Ok,
            Some(vec![make_step("mod1.fn1.s1", "s1", "intent")]),
        )],
    )]);
    let next = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "",
        vec![make_fn("mod1.fn1", "fn1", Status::Ok, Some(vec![]))],
    )]);

    let patch = diff_graph_at(&prev, &next, 0);

    assert_eq!(patch.steps_removed.len(), 1, "expected 1 removed step");
    assert_eq!(patch.steps_removed[0].function_id, "mod1.fn1");
    assert_eq!(patch.steps_removed[0].step_id, "mod1.fn1.s1");
}

// ---------------------------------------------------------------------------
// Identity and timestamp tests
// ---------------------------------------------------------------------------

/// Test 10: identical graphs → all 9 arrays empty, timestamp == 0.
#[test]
fn test_diff_empty_when_graphs_identical() {
    let graph = graph_with_modules(vec![make_module(
        "mod1",
        "m1",
        "desc",
        vec![make_fn(
            "mod1.fn1",
            "fn1",
            Status::Ok,
            Some(vec![make_step("mod1.fn1.s1", "s1", "intent")]),
        )],
    )]);

    let patch = diff_graph_at(&graph, &graph, 0);

    assert!(patch.modules_added.is_empty());
    assert!(patch.modules_modified.is_empty());
    assert!(patch.modules_removed.is_empty());
    assert!(patch.functions_added.is_empty());
    assert!(patch.functions_modified.is_empty());
    assert!(patch.functions_removed.is_empty());
    assert!(patch.steps_added.is_empty());
    assert!(patch.steps_modified.is_empty());
    assert!(patch.steps_removed.is_empty());
    assert_eq!(patch.timestamp, 0);
}

/// Test 11: timestamp is preserved via diff_graph_at.
#[test]
fn test_diff_timestamp_preserved_via_at() {
    let g = empty_graph();
    let patch = diff_graph_at(&g, &g, 42);
    assert_eq!(patch.timestamp, 42);
}

/// Test 12: GraphPatchJson with entries in every array roundtrips via serde.
#[test]
fn test_patch_json_roundtrip() {
    use ail_ui_bridge::types::patch::{
        FunctionPatchEntry, FunctionRemoval, StepPatchEntry, StepRemoval,
    };

    let original = GraphPatchJson {
        modules_added: vec![make_module("m1", "M1", "d", vec![])],
        modules_modified: vec![make_module("m2", "M2", "d2", vec![])],
        modules_removed: vec!["m3".to_string()],
        functions_added: vec![FunctionPatchEntry {
            module_id: "m1".to_string(),
            function: make_fn("m1.f1", "f1", Status::Ok, None),
        }],
        functions_modified: vec![FunctionPatchEntry {
            module_id: "m2".to_string(),
            function: make_fn("m2.f2", "f2", Status::Warn, None),
        }],
        functions_removed: vec![FunctionRemoval {
            module_id: "m3".to_string(),
            function_id: "m3.f3".to_string(),
        }],
        steps_added: vec![StepPatchEntry {
            function_id: "m1.f1".to_string(),
            step: make_step("m1.f1.s1", "s1", "intent A"),
        }],
        steps_modified: vec![StepPatchEntry {
            function_id: "m2.f2".to_string(),
            step: make_step("m2.f2.s2", "s2", "intent B"),
        }],
        steps_removed: vec![StepRemoval {
            function_id: "m3.f3".to_string(),
            step_id: "m3.f3.s3".to_string(),
        }],
        timestamp: 9999,
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: GraphPatchJson = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original, restored, "roundtrip must preserve equality");
    assert_eq!(restored.timestamp, 9999);
    assert_eq!(restored.modules_added[0].id, "m1");
    assert_eq!(restored.functions_added[0].module_id, "m1");
    assert_eq!(restored.steps_added[0].function_id, "m1.f1");
    assert_eq!(restored.steps_removed[0].step_id, "m3.f3.s3");
}

// ---------------------------------------------------------------------------
// Helper assertions
// ---------------------------------------------------------------------------

fn assert_all_fn_and_step_arrays_empty(patch: &GraphPatchJson) {
    assert!(
        patch.functions_added.is_empty(),
        "functions_added must be empty"
    );
    assert!(
        patch.functions_modified.is_empty(),
        "functions_modified must be empty"
    );
    assert!(
        patch.functions_removed.is_empty(),
        "functions_removed must be empty"
    );
    assert!(patch.steps_added.is_empty(), "steps_added must be empty");
    assert!(
        patch.steps_modified.is_empty(),
        "steps_modified must be empty"
    );
    assert!(
        patch.steps_removed.is_empty(),
        "steps_removed must be empty"
    );
}
