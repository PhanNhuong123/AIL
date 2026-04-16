use ail_contract::verify;
use ail_emit::{emit_ts_test_definitions, EmitConfig, TestFramework};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Param, Pattern,
};
use ail_types::type_check;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a `VerifiedGraph` containing:
/// - A root `Describe` node.
/// - Zero or more type nodes (Define / Describe / Error) as children of the root.
/// - One top-level `Do` node with the given params and contracts.
/// - Zero or more `Check` children attached to the Do node, each carrying `otherwise_error`.
///
/// Validation rule v005 requires at least one Before AND one After contract on every
/// top-level Do node. If either is absent, a pass-through dummy (`true == true`) is
/// appended automatically so the test graph passes without affecting the stubs being
/// asserted.
fn build_test_graph(
    fn_name: &str,
    params: Vec<(&str, &str)>,
    mut contracts: Vec<Contract>,
    type_nodes: Vec<(Pattern, &str)>,
    check_errors: Vec<&str>,
) -> ail_contract::VerifiedGraph {
    // Auto-pad dummies to satisfy v005.
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

    // Root Describe node.
    let root_id = NodeId::new();
    let root = Node {
        id: root_id,
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    let mut root_children: Vec<NodeId> = Vec::new();

    // Type nodes as children of root.
    for (pattern, name) in &type_nodes {
        let type_id = NodeId::new();
        let mut type_node = Node {
            id: type_id,
            intent: name.to_string(),
            pattern: pattern.clone(),
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        type_node.metadata.name = Some(name.to_string());
        graph.add_node(type_node).expect("add type node");
        graph
            .add_edge(root_id, type_id, EdgeKind::Ev)
            .expect("root→type");
        root_children.push(type_id);
    }

    // Do node.
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
    fn_node.metadata.return_type = Some("number".to_owned());
    graph.add_node(fn_node).expect("add fn node");
    graph
        .add_edge(root_id, fn_id, EdgeKind::Ev)
        .expect("root→fn");
    root_children.push(fn_id);

    // Check children of the Do node.
    let mut check_ids: Vec<NodeId> = Vec::new();
    for err_type in &check_errors {
        let check_id = NodeId::new();
        let mut check_node = Node {
            id: check_id,
            intent: format!("check for {err_type}"),
            pattern: Pattern::Check,
            children: None,
            expression: Some(Expression("x > 0".to_owned())),
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        check_node.metadata.otherwise_error = Some(err_type.to_string());
        graph.add_node(check_node).expect("add check node");
        graph
            .add_edge(fn_id, check_id, EdgeKind::Ev)
            .expect("fn→check");
        check_ids.push(check_id);
    }

    // Wire sibling edges for check children.
    for i in 0..check_ids.len().saturating_sub(1) {
        graph
            .add_edge(check_ids[i], check_ids[i + 1], EdgeKind::Eh)
            .expect("Eh sibling");
    }

    if !check_ids.is_empty() {
        graph.get_node_mut(fn_id).expect("fn").children = Some(check_ids);
    }

    // Wire sibling edges for root children.
    for i in 0..root_children.len().saturating_sub(1) {
        graph
            .add_edge(root_children[i], root_children[i + 1], EdgeKind::Eh)
            .expect("Eh root sibling");
    }
    graph.get_node_mut(root_id).expect("root").children = Some(root_children);

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

// ── Happy path ────────────────────────────────────────────────────────────────

#[test]
fn t094_generates_happy_path_test() {
    let verified = build_test_graph("pay", vec![("amount", "number")], vec![], vec![], vec![]);
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/pay.test.ts");
    assert!(
        content.contains("should succeed with valid inputs"),
        "happy-path test name missing:\n{content}"
    );
    // Real test body — not a stub.
    assert!(
        !content.contains("it.todo('should succeed"),
        "happy-path must not be a todo stub:\n{content}"
    );
    assert!(
        content.contains("expect(result).toBeDefined()"),
        "happy-path body assertion missing:\n{content}"
    );
}

// ── Precondition violation ────────────────────────────────────────────────────

#[test]
fn t094_generates_precondition_violation_test() {
    let verified = build_test_graph(
        "validate",
        vec![("x", "number")],
        vec![make_contract(ContractKind::Before, "x > 0")],
        vec![],
        vec![],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/validate.test.ts");
    assert!(
        content.contains("precondition violated: x > 0"),
        "precondition violation test name missing:\n{content}"
    );
    // Real test body — not a stub. Function is sync, param is primitive.
    assert!(
        !content.contains("it.todo('should throw when precondition"),
        "precondition violation must not be a todo stub:\n{content}"
    );
    assert!(
        content.contains("expect(() => validate(0)).toThrow()"),
        "precondition violation body missing:\n{content}"
    );
}

// ── Postcondition ─────────────────────────────────────────────────────────────

#[test]
fn t094_generates_postcondition_stub() {
    let verified = build_test_graph(
        "compute",
        vec![("x", "number")],
        vec![make_contract(ContractKind::After, "result > 0")],
        vec![],
        vec![],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/compute.test.ts");
    assert!(
        content.contains("should satisfy postcondition: result > 0"),
        "postcondition test name missing:\n{content}"
    );
    // Dummy padding must NOT generate an extra postcondition test.
    assert_eq!(
        content.matches("satisfy postcondition").count(),
        1,
        "dummy after contract generated extra postcondition test:\n{content}"
    );
    // Real test body — not a stub.
    assert!(
        content.contains("expect(result).toBeDefined()"),
        "postcondition body assertion missing:\n{content}"
    );
}

// ── Error path ────────────────────────────────────────────────────────────────

#[test]
fn t094_generates_error_path_test_per_check() {
    let verified = build_test_graph(
        "transfer",
        vec![("amount", "number")],
        vec![],
        vec![],
        vec!["InsufficientBalanceError"],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/transfer.test.ts");
    assert!(
        content.contains("throw InsufficientBalanceError when check fails"),
        "error-path test name missing:\n{content}"
    );
    // Error-path tests use it.skip (triggering the check requires constraint solving).
    assert!(
        content.contains("it.skip("),
        "error-path must use it.skip:\n{content}"
    );
    // Must NOT be a runnable it() body (which would pass without actually triggering the error).
    assert!(
        !content.contains("it('should throw InsufficientBalanceError"),
        "error-path must not be a runnable it() body:\n{content}"
    );
}

// ── Invariant ─────────────────────────────────────────────────────────────────

#[test]
fn t094_generates_invariant_test() {
    let verified = build_test_graph(
        "withdraw",
        vec![("balance", "number")],
        vec![make_contract(ContractKind::Always, "balance >= 0")],
        vec![],
        vec![],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/withdraw.test.ts");
    assert!(
        content.contains("should maintain invariant: balance >= 0"),
        "invariant test name missing:\n{content}"
    );
    // Real test body — not a stub.
    assert!(
        content.contains("expect(result).toBeDefined()"),
        "invariant body assertion missing:\n{content}"
    );
}

// ── Boundary value ────────────────────────────────────────────────────────────

#[test]
fn t094_generates_boundary_value_test() {
    let verified = build_test_graph(
        "process",
        vec![("x", "number")],
        vec![make_contract(ContractKind::Before, "x >= 0")],
        vec![],
        vec![],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/process.test.ts");
    assert!(
        content.contains("boundary: x >= 0"),
        "boundary stub missing:\n{content}"
    );
}

// ── Framework: vitest ─────────────────────────────────────────────────────────

#[test]
fn t094_test_uses_vitest_syntax() {
    let verified = build_test_graph("pay", vec![], vec![], vec![], vec![]);
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/pay.test.ts");
    assert!(
        content.contains("from 'vitest'"),
        "vitest import missing:\n{content}"
    );
    assert!(
        content.contains("describe, it, expect"),
        "describe/it/expect import missing:\n{content}"
    );
}

// ── Framework: jest ───────────────────────────────────────────────────────────

#[test]
fn t094_test_uses_jest_syntax_when_configured() {
    let verified = build_test_graph("pay", vec![], vec![], vec![], vec![]);
    let config = EmitConfig {
        test_framework: TestFramework::Jest,
        ..Default::default()
    };
    let output = emit_ts_test_definitions(&verified, &config);
    let content = file_content(&output, "tests/pay.test.ts");
    assert!(
        content.contains("from '@jest/globals'"),
        "@jest/globals import missing:\n{content}"
    );
    assert!(
        !content.contains("from 'vitest'"),
        "vitest import must be absent in jest mode:\n{content}"
    );
}

// ── Type imports ──────────────────────────────────────────────────────────────

#[test]
fn t094_test_imports_all_required_types() {
    // A Define type node named "Amount" + a param typed as Amount.
    let verified = build_test_graph(
        "pay",
        vec![("amount", "Amount")],
        vec![],
        vec![(Pattern::Define, "Amount")],
        vec![],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/pay.test.ts");
    assert!(
        content.contains("from '../types/amount'"),
        "../types/amount import missing:\n{content}"
    );
    assert!(
        content.contains("Amount"),
        "Amount symbol missing:\n{content}"
    );
    assert!(
        content.contains("createAmount"),
        "createAmount factory import missing:\n{content}"
    );
}

// ── Error imports ─────────────────────────────────────────────────────────────

#[test]
fn t094_test_imports_all_required_errors() {
    let verified = build_test_graph(
        "transfer",
        vec![("amount", "number")],
        vec![],
        vec![],
        vec!["InsufficientBalanceError"],
    );
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/transfer.test.ts");
    assert!(
        content.contains("from '../errors/insufficient_balance_error'"),
        "../errors/ import missing:\n{content}"
    );
    assert!(
        content.contains("InsufficientBalanceError"),
        "error class import missing:\n{content}"
    );
}

// ── File naming convention ────────────────────────────────────────────────────

#[test]
fn t094_test_file_naming_convention() {
    let verified = build_test_graph("transfer money", vec![], vec![], vec![], vec![]);
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    assert!(
        output
            .files
            .iter()
            .any(|f| f.path == "tests/transfer_money.test.ts"),
        "expected tests/transfer_money.test.ts, found: {:?}",
        output.files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );
}

// ── Minimal output for no contracts (9.4-D) ──────────────────────────────────

#[test]
fn t094_generates_minimal_stub_for_no_contracts() {
    // A simple Do node with no contracts and no Check children.
    let verified = build_test_graph("simple_fn", vec![], vec![], vec![], vec![]);
    let output = emit_ts_test_definitions(&verified, &EmitConfig::default());
    let content = file_content(&output, "tests/simple_fn.test.ts");

    // Happy path with real body is always present.
    assert!(
        content.contains("should succeed with valid inputs"),
        "happy-path test missing:\n{content}"
    );
    assert!(
        content.contains("expect(result).toBeDefined()"),
        "happy-path body assertion missing:\n{content}"
    );

    // No precondition, postcondition, invariant, or error tests.
    assert!(
        !content.contains("precondition violated"),
        "unexpected precondition test:\n{content}"
    );
    assert!(
        !content.contains("satisfy postcondition"),
        "unexpected postcondition test:\n{content}"
    );
    assert!(
        !content.contains("maintain invariant"),
        "unexpected invariant test:\n{content}"
    );
    assert!(
        !content.contains("when check fails"),
        "unexpected error-path test:\n{content}"
    );
}
