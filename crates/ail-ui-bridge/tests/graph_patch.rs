use ail_ui_bridge::serialize::diff_graph;
use ail_ui_bridge::types::graph_json::{FunctionJson, GraphJson, ModuleJson, ProjectJson};
use ail_ui_bridge::types::patch::PatchItem;
use ail_ui_bridge::types::status::Status;
use std::collections::BTreeMap;

fn minimal_graph_with_fns(fns: Vec<FunctionJson>) -> GraphJson {
    let fn_count = fns.len();
    GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: fn_count,
            module_count: 1,
            fn_count,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules: vec![ModuleJson {
            id: "mod1".to_string(),
            name: "mod1".to_string(),
            description: String::new(),
            cluster: "default".to_string(),
            cluster_name: "test".to_string(),
            cluster_color: "#2997ff".to_string(),
            status: Status::Ok,
            node_count: fn_count,
            functions: fns,
        }],
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        detail: BTreeMap::new(),
    }
}

fn make_fn(id: &str, name: &str, status: Status) -> FunctionJson {
    FunctionJson {
        id: id.to_string(),
        name: name.to_string(),
        status,
        steps: None,
    }
}

/// Test 9: diff detects an added function.
#[test]
fn test_graph_patch_add() {
    let prev = minimal_graph_with_fns(vec![make_fn("mod1.fn_a", "fn_a", Status::Ok)]);
    let next = minimal_graph_with_fns(vec![
        make_fn("mod1.fn_a", "fn_a", Status::Ok),
        make_fn("mod1.fn_b", "fn_b", Status::Ok),
    ]);

    let patch = diff_graph(&prev, &next);

    assert_eq!(patch.added.len(), 1, "expected 1 added item");
    assert!(
        matches!(&patch.added[0], PatchItem::Function(f) if f.id == "mod1.fn_b"),
        "added item must be fn_b"
    );
    assert!(patch.modified.is_empty(), "expected no modified items");
    assert!(patch.removed.is_empty(), "expected no removed items");
}

/// Test 10: diff detects a modified function (status change).
#[test]
fn test_graph_patch_modify() {
    let prev = minimal_graph_with_fns(vec![make_fn("mod1.fn_a", "fn_a", Status::Ok)]);
    let next = minimal_graph_with_fns(vec![make_fn("mod1.fn_a", "fn_a", Status::Fail)]);

    let patch = diff_graph(&prev, &next);

    assert_eq!(patch.modified.len(), 1, "expected 1 modified item");
    assert!(
        matches!(&patch.modified[0], PatchItem::Function(f) if f.status == Status::Fail),
        "modified item must have status Fail"
    );
    assert!(patch.added.is_empty(), "expected no added items");
    assert!(patch.removed.is_empty(), "expected no removed items");
}

/// Test 11: diff detects a removed function.
#[test]
fn test_graph_patch_remove() {
    let prev = minimal_graph_with_fns(vec![
        make_fn("mod1.fn_a", "fn_a", Status::Ok),
        make_fn("mod1.fn_b", "fn_b", Status::Ok),
    ]);
    let next = minimal_graph_with_fns(vec![make_fn("mod1.fn_a", "fn_a", Status::Ok)]);

    let patch = diff_graph(&prev, &next);

    assert!(
        patch.removed.contains(&"mod1.fn_b".to_string()),
        "fn_b must be removed"
    );
    assert!(patch.added.is_empty(), "expected no added items");
    assert!(patch.modified.is_empty(), "expected no modified items");
}
