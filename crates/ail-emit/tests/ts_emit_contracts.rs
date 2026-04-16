use ail_contract::verify;
use ail_emit::{emit_ts_function_definitions, ContractMode, EmitConfig};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Param, Pattern,
};
use ail_types::type_check;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a verified graph with a single top-level Do node and given contracts.
///
/// Validation rule v005 requires top-level Do nodes to have at least one Before
/// AND one After contract. If the caller does not supply one of those kinds,
/// a pass-through dummy (`true == true`) is appended automatically so that the
/// test graph passes validation without affecting the contract-emission output
/// the test is actually asserting on.
fn build_contract_graph(
    fn_name: &str,
    params: Vec<(&str, &str)>,
    return_type: &str,
    mut contracts: Vec<Contract>,
    children: Vec<Node>,
) -> ail_contract::VerifiedGraph {
    // Pad with dummies as needed to satisfy v005.
    let has_before = contracts.iter().any(|c| c.kind == ContractKind::Before);
    let has_after = contracts.iter().any(|c| c.kind == ContractKind::After);
    if !has_before {
        contracts.push(Contract {
            kind: ContractKind::Before,
            expression: Expression("true == true".to_owned()),
        });
    }
    if !has_after {
        contracts.push(Contract {
            kind: ContractKind::After,
            expression: Expression("true == true".to_owned()),
        });
    }

    let mut graph = AilGraph::new();

    let root = Node {
        id: NodeId::new(),
        intent: "root container".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    let fn_id = NodeId::new();
    let mut fn_node = Node {
        id: fn_id,
        intent: fn_name.to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts,
        metadata: NodeMetadata::default(),
    };
    fn_node.metadata.name = Some(fn_name.to_owned());
    fn_node.metadata.params = params
        .into_iter()
        .map(|(n, t)| Param {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    fn_node.metadata.return_type = Some(return_type.to_owned());

    graph.add_node(fn_node).expect("add fn node");
    graph
        .add_edge(root_id, fn_id, EdgeKind::Ev)
        .expect("root→fn");

    let mut child_ids: Vec<NodeId> = Vec::new();
    for node in children {
        let child_id = node.id;
        graph.add_node(node).expect("add child");
        graph
            .add_edge(fn_id, child_id, EdgeKind::Ev)
            .expect("fn→child");
        child_ids.push(child_id);
    }
    for i in 0..child_ids.len().saturating_sub(1) {
        graph
            .add_edge(child_ids[i], child_ids[i + 1], EdgeKind::Eh)
            .expect("Eh sibling");
    }
    if !child_ids.is_empty() {
        graph.get_node_mut(fn_id).expect("fn").children = Some(child_ids);
    }
    graph.get_node_mut(root_id).expect("root").children = Some(vec![fn_id]);

    let valid = validate_graph(graph).expect("validate");
    let typed = type_check(valid, &[]).expect("type_check");
    verify(typed).expect("verify")
}

fn make_contract(kind: ContractKind, expr: &str) -> Contract {
    Contract {
        kind,
        expression: Expression(expr.to_owned()),
    }
}

fn return_node(type_name: &str) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("return {type_name}"),
        pattern: Pattern::Return,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(type_name.to_owned());
    node
}

fn config(mode: ContractMode) -> EmitConfig {
    EmitConfig {
        contract_mode: mode,
        async_mode: false,
        ..Default::default()
    }
}

fn file_content<'a>(output: &'a ail_emit::EmitOutput, path: &str) -> &'a str {
    output
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| {
            panic!(
                "file not found: {path}\navailable: {:?}",
                output.files.iter().map(|f| &f.path).collect::<Vec<_>>()
            )
        })
        .content
        .as_str()
}

// ── Mode: On ──────────────────────────────────────────────────────────────────

#[test]
fn t093_contracts_on_emits_pre() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "void",
        vec![make_contract(ContractKind::Before, "x > 0")],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(content.contains("pre("), "pre() call missing:\n{content}");
    assert!(
        content.contains("\"x > 0\""),
        "raw expression missing:\n{content}"
    );
}

#[test]
fn t093_contracts_on_emits_post() {
    let verified = build_contract_graph(
        "compute",
        vec![("x", "number")],
        "number",
        vec![make_contract(ContractKind::After, "result > 0")],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/compute.ts");
    assert!(content.contains("post("), "post() call missing:\n{content}");
    // post() must appear before the return statement.
    let post_pos = content.find("post(").expect("post( not found");
    let return_pos = content.find("return ").expect("return not found");
    assert!(
        post_pos < return_pos,
        "post() must appear before return:\n{content}"
    );
}

#[test]
fn t093_contracts_on_emits_keep() {
    let verified = build_contract_graph(
        "transfer",
        vec![("balance", "number")],
        "void",
        vec![make_contract(ContractKind::Always, "balance >= 0")],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/transfer.ts");
    assert!(content.contains("keep("), "keep() call missing:\n{content}");
}

// ── Mode: Comments ────────────────────────────────────────────────────────────

#[test]
fn t093_contracts_comments_emits_comments_only() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "number",
        vec![
            make_contract(ContractKind::Before, "x > 0"),
            make_contract(ContractKind::After, "result > 0"),
            make_contract(ContractKind::Always, "x >= 0"),
        ],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::Comments)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        content.contains("// PRE:"),
        "// PRE: comment missing:\n{content}"
    );
    assert!(
        content.contains("// POST:"),
        "// POST: comment missing:\n{content}"
    );
    assert!(
        content.contains("// KEEP:"),
        "// KEEP: comment missing:\n{content}"
    );
    // No live runtime calls.
    assert!(
        !content.contains("pre("),
        "pre() should not be present in comments mode:\n{content}"
    );
    assert!(
        !content.contains("post("),
        "post() should not be present in comments mode:\n{content}"
    );
    assert!(
        !content.contains("keep("),
        "keep() should not be present in comments mode:\n{content}"
    );
}

// ── Mode: Off ─────────────────────────────────────────────────────────────────

#[test]
fn t093_contracts_off_emits_nothing() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "void",
        vec![
            make_contract(ContractKind::Before, "x > 0"),
            make_contract(ContractKind::After, "result > 0"),
            make_contract(ContractKind::Always, "x >= 0"),
        ],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::Off)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        !content.contains("pre("),
        "pre() must be absent:\n{content}"
    );
    assert!(
        !content.contains("post("),
        "post() must be absent:\n{content}"
    );
    assert!(
        !content.contains("keep("),
        "keep() must be absent:\n{content}"
    );
    assert!(
        !content.contains("../ail-runtime"),
        "runtime import must be absent:\n{content}"
    );
}

// ── Mode: Test ────────────────────────────────────────────────────────────────

#[test]
fn t093_contracts_test_emits_in_test_file_only() {
    // mode=Test → fn files behave as Off (contracts appear in test files only, which is 9.4).
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "void",
        vec![make_contract(ContractKind::Before, "x > 0")],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::Test)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        !content.contains("pre("),
        "pre() must be absent in fn file for test mode:\n{content}"
    );
    assert!(
        !content.contains("../ail-runtime"),
        "runtime import must be absent:\n{content}"
    );
}

// ── old() snapshots ───────────────────────────────────────────────────────────

#[test]
fn t093_old_expression_captured_at_entry() {
    let verified = build_contract_graph(
        "transfer",
        vec![("x", "number"), ("y", "number")],
        "number",
        vec![make_contract(
            ContractKind::After,
            "result is old(x.value) - y",
        )],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/transfer.ts");
    // Snapshot line must be present.
    assert!(
        content.contains("const _old_x_value = x.value;"),
        "old() snapshot missing:\n{content}"
    );
    // Snapshot must appear BEFORE the body (i.e., before the return statement).
    let snapshot_pos = content
        .find("_old_x_value = x.value")
        .expect("snapshot not found");
    let return_pos = content.find("return ").expect("return not found");
    assert!(
        snapshot_pos < return_pos,
        "snapshot must be captured before body:\n{content}"
    );
}

#[test]
fn t093_old_nested_field_captured() {
    let verified = build_contract_graph(
        "process",
        vec![("a", "number")],
        "number",
        vec![make_contract(
            ContractKind::After,
            "result is old(a.b.c) + 1",
        )],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/process.ts");
    assert!(
        content.contains("_old_a_b_c"),
        "nested old() snapshot var missing:\n{content}"
    );
}

// ── Keep (Always) before and after body ──────────────────────────────────────

#[test]
fn t093_keep_emits_before_and_after_body() {
    let verified = build_contract_graph(
        "transfer",
        vec![("balance", "number")],
        "number",
        vec![make_contract(ContractKind::Always, "balance >= 0")],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/transfer.ts");

    // keep() must appear at least twice — once before body, once before return.
    let count = content.matches("keep(").count();
    assert!(
        count >= 2,
        "keep() should appear before body and before return, found {count} occurrences:\n{content}"
    );
}

// ── Multiple contracts ────────────────────────────────────────────────────────

#[test]
fn t093_multiple_pre_conditions_all_emitted() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number"), ("y", "number")],
        "void",
        vec![
            make_contract(ContractKind::Before, "x > 0"),
            make_contract(ContractKind::Before, "y > 0"),
        ],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    let count = content.matches("pre(").count();
    assert_eq!(
        count, 2,
        "expected 2 pre() calls, found {count}:\n{content}"
    );
}

#[test]
fn t093_multiple_post_conditions_all_emitted() {
    let verified = build_contract_graph(
        "compute",
        vec![("x", "number"), ("y", "number")],
        "number",
        vec![
            make_contract(ContractKind::After, "result > 0"),
            make_contract(ContractKind::After, "result < 100"),
        ],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/compute.ts");
    let count = content.matches("post(").count();
    assert_eq!(
        count, 2,
        "expected 2 post() calls, found {count}:\n{content}"
    );
}

// ── Runtime import ────────────────────────────────────────────────────────────

#[test]
fn t093_import_pre_post_keep_from_runtime() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "void",
        vec![make_contract(ContractKind::Before, "x > 0")],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        content.contains("import { pre, post, keep } from '../ail-runtime'"),
        "runtime import missing:\n{content}"
    );
}

#[test]
fn t093_import_not_added_when_contracts_off() {
    let verified = build_contract_graph(
        "validate",
        vec![("x", "number")],
        "void",
        vec![make_contract(ContractKind::Before, "x > 0")],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::Off)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(
        !content.contains("../ail-runtime"),
        "runtime import must be absent when mode=Off:\n{content}"
    );
}

// ── pre() call structure ──────────────────────────────────────────────────────

#[test]
fn t093_precondition_error_class_used() {
    // pre(expr, "raw") — the runtime pre() call is present in the function body.
    let verified = build_contract_graph(
        "validate",
        vec![("sender_status", "text")],
        "void",
        vec![make_contract(
            ContractKind::Before,
            "sender_status is \"active\"",
        )],
        vec![],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/validate.ts");
    assert!(content.contains("pre("), "pre() call missing:\n{content}");
    assert!(
        content.contains("sender_status === 'active'"),
        "rendered TS expression missing:\n{content}"
    );
}

// ── Invariant after body ──────────────────────────────────────────────────────

#[test]
fn t093_invariant_check_after_body() {
    // Always contract → keep() appears after the body (before return).
    let verified = build_contract_graph(
        "withdraw",
        vec![("balance", "number"), ("amount", "number")],
        "number",
        vec![make_contract(ContractKind::Always, "balance >= 0")],
        vec![return_node("number")],
    );
    let output = emit_ts_function_definitions(&verified, &config(ContractMode::On)).unwrap();
    let content = file_content(&output, "fn/withdraw.ts");

    // keep() must appear before the return statement (after body).
    let keep_pos = content.rfind("keep(").expect("keep( not found");
    let return_pos = content.rfind("return ").expect("return not found");
    assert!(
        keep_pos < return_pos,
        "keep() must appear before return (after body):\n{content}"
    );
}
