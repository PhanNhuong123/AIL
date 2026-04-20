use ail_ui_bridge::rollup::{rollup, rollup_from_contracts};
use ail_ui_bridge::types::graph_json::{FunctionJson, GraphJson, ModuleJson, ProjectJson};
use ail_ui_bridge::types::status::Status;
use std::collections::BTreeMap;

fn make_minimal_graph(module_statuses: Vec<Status>) -> GraphJson {
    let functions: Vec<FunctionJson> = module_statuses
        .iter()
        .enumerate()
        .map(|(i, &s)| FunctionJson {
            id: format!("mod.fn_{i}"),
            name: format!("fn_{i}"),
            status: s,
            steps: None,
        })
        .collect();

    let module_status = rollup(&module_statuses);

    GraphJson {
        project: ProjectJson {
            id: "test".to_string(),
            name: "test".to_string(),
            description: String::new(),
            node_count: functions.len(),
            module_count: 1,
            fn_count: functions.len(),
            status: module_status,
        },
        clusters: Vec::new(),
        modules: vec![ModuleJson {
            id: "mod".to_string(),
            name: "mod".to_string(),
            description: String::new(),
            cluster: "default".to_string(),
            cluster_name: "test".to_string(),
            cluster_color: "#2997ff".to_string(),
            status: module_status,
            node_count: functions.len(),
            functions,
        }],
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        detail: BTreeMap::new(),
    }
}

/// Test 3: rollup surfaces Fail from mixed function statuses.
#[test]
fn test_status_rollup_module() {
    let statuses = vec![Status::Ok, Status::Warn, Status::Fail];
    let graph = make_minimal_graph(statuses);
    assert_eq!(graph.modules[0].status, Status::Fail);
    assert_eq!(graph.project.status, Status::Fail);
}

/// Test 4: rollup handles empty and mixed.
#[test]
fn test_status_rollup_function() {
    // Empty → Ok
    assert_eq!(rollup(&[]), Status::Ok);

    // Mixed with Fail → Fail
    assert_eq!(rollup(&[Status::Ok, Status::Fail]), Status::Fail);

    // Mixed without Fail → Warn
    assert_eq!(rollup(&[Status::Ok, Status::Warn]), Status::Warn);

    // rollup_from_contracts
    assert_eq!(rollup_from_contracts(false), Status::Ok);
    assert_eq!(rollup_from_contracts(true), Status::Fail);
}

/// Exhaustive unit tests for rollup (embedded inline tests cover this too,
/// but integration tests confirm the public API).
#[test]
fn test_rollup_unit_exhaustive() {
    use Status::{Fail, Ok, Warn};

    assert_eq!(rollup(&[]), Ok);
    assert_eq!(rollup(&[Ok]), Ok);
    assert_eq!(rollup(&[Warn]), Warn);
    assert_eq!(rollup(&[Fail]), Fail);
    assert_eq!(rollup(&[Ok, Warn]), Warn);
    assert_eq!(rollup(&[Ok, Fail]), Fail);
    assert_eq!(rollup(&[Warn, Fail]), Fail);
    assert_eq!(rollup(&[Ok, Warn, Fail]), Fail);
    assert_eq!(rollup(&[Ok, Ok]), Ok);
    assert_eq!(rollup(&[Warn, Warn]), Warn);
    assert_eq!(rollup(&[Fail, Fail]), Fail);
}
