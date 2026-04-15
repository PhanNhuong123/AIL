use ail_contract::verify;
use ail_emit::{emit_type_definitions, EmitError, FileOwnership};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId,
    NodeMetadata, Pattern,
};
use ail_types::type_check;

// ── Test helpers ─────────────────────────────────────────────────────────────

/// Build a minimal verified graph containing only type-defining nodes.
///
/// Creates a root container (Describe with name=None), connects all nodes
/// as children via Ev edges, and links siblings via Eh edges.
fn build_verified_types_graph(nodes: Vec<Node>) -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();

    // Create a root container node.
    let root = Node {
        id: NodeId::new(),
        intent: "root container".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).expect("add root");
    graph.set_root(root_id).expect("set root");

    // Add child nodes and connect them.
    let mut child_ids = Vec::new();
    for node in nodes {
        let child_id = node.id;
        graph.add_node(node).expect("add child node");
        // Ev edge: root → child.
        graph
            .add_edge(root_id, child_id, EdgeKind::Ev)
            .expect("add Ev edge");
        child_ids.push(child_id);
    }

    // Eh edges: sibling chain.
    for i in 0..child_ids.len().saturating_sub(1) {
        graph
            .add_edge(child_ids[i], child_ids[i + 1], EdgeKind::Eh)
            .expect("add Eh edge");
    }

    // Update root's children list.
    if !child_ids.is_empty() {
        let root_node = graph.get_node_mut(root_id).expect("root exists");
        root_node.children = Some(child_ids);
    }

    let valid = validate_graph(graph).expect("validation should pass");
    let typed = type_check(valid, &[]).expect("type check should pass");
    verify(typed).expect("verification should pass")
}

fn define_node(name: &str, base: &str, constraint: Option<&str>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} type definition"),
        pattern: Pattern::Define,
        children: None,
        expression: constraint.map(|c| Expression(c.to_owned())),
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.base_type = Some(base.to_owned());
    if let Some(c) = constraint {
        node.contracts.push(Contract {
            kind: ContractKind::Always,
            expression: Expression(c.to_owned()),
        });
    }
    node
}

fn describe_node(name: &str, fields: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} record type"),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.fields = fields
        .into_iter()
        .map(|(n, t)| Field {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    node
}

fn error_node(name: &str, carries: Vec<(&str, &str)>) -> Node {
    let mut node = Node {
        id: NodeId::new(),
        intent: format!("{name} error type"),
        pattern: Pattern::Error,
        children: None,
        expression: None,
        contracts: Vec::new(),
        metadata: NodeMetadata::default(),
    };
    node.metadata.name = Some(name.to_owned());
    node.metadata.carries = carries
        .into_iter()
        .map(|(n, t)| Field {
            name: n.to_owned(),
            type_ref: t.to_owned(),
        })
        .collect();
    node
}

// ── Integration tests ────────────────────────────────────────────────────────

#[test]
fn emit_wallet_domain_types() {
    let verified = build_verified_types_graph(vec![
        define_node("WalletBalance", "number", Some("value >= 0")),
        define_node("PositiveAmount", "number", Some("value > 0")),
        define_node("UserId", "text", None),
        define_node("UserStatus", "text", None),
        describe_node(
            "User",
            vec![
                ("id", "UserId"),
                ("balance", "WalletBalance"),
                ("status", "UserStatus"),
            ],
        ),
        error_node(
            "InsufficientBalanceError",
            vec![
                ("current_balance", "WalletBalance"),
                ("requested_amount", "PositiveAmount"),
            ],
        ),
    ]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    assert_eq!(output.files.len(), 1);

    let file = &output.files[0];
    assert_eq!(file.path, "generated/types.py");

    let content = &file.content;

    // Check imports.
    assert!(content.contains("from __future__ import annotations"));
    assert!(content.contains("from dataclasses import dataclass"));
    assert!(content.contains("from ail_runtime import keep"));

    // Check all types are present.
    assert!(content.contains("class WalletBalance:"));
    assert!(content.contains("class PositiveAmount:"));
    assert!(content.contains("class User:"));
    assert!(content.contains("class InsufficientBalanceError(Exception):"));
}

#[test]
fn emit_empty_graph_produces_no_output() {
    let verified = build_verified_types_graph(vec![]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    assert!(output.files.is_empty());
}

#[test]
fn emit_imports_collected_correctly() {
    // Only a Describe node → needs dataclass but not keep or re.
    let verified = build_verified_types_graph(vec![describe_node(
        "Point",
        vec![("x", "number"), ("y", "number")],
    )]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    let content = &output.files[0].content;

    assert!(content.contains("from __future__ import annotations"));
    assert!(content.contains("from dataclasses import dataclass"));
    assert!(!content.contains("from ail_runtime import keep"));
    assert!(!content.contains("import re"));
}

#[test]
fn emit_ordering_define_before_describe_before_error() {
    let verified = build_verified_types_graph(vec![
        // Insert in mixed order: error, describe, define.
        error_node("SomeError", vec![]),
        describe_node("SomeRecord", vec![("x", "integer")]),
        define_node("SomeType", "text", None),
    ]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    let content = &output.files[0].content;

    let define_pos = content.find("class SomeType:").expect("SomeType not found");
    let describe_pos = content
        .find("class SomeRecord:")
        .expect("SomeRecord not found");
    let error_pos = content
        .find("class SomeError(Exception):")
        .expect("SomeError not found");

    assert!(
        define_pos < describe_pos,
        "Define should appear before Describe"
    );
    assert!(
        describe_pos < error_pos,
        "Describe should appear before Error"
    );
}

#[test]
fn emit_describe_field_order_preserved() {
    let verified = build_verified_types_graph(vec![describe_node(
        "Ordered",
        vec![
            ("z_last", "text"),
            ("a_first", "text"),
            ("m_middle", "text"),
        ],
    )]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    let content = &output.files[0].content;

    let z_pos = content.find("z_last: str").expect("z_last not found");
    let a_pos = content.find("a_first: str").expect("a_first not found");
    let m_pos = content.find("m_middle: str").expect("m_middle not found");

    assert!(z_pos < a_pos, "z_last should appear before a_first");
    assert!(a_pos < m_pos, "a_first should appear before m_middle");
}

#[test]
fn emit_accumulated_errors() {
    // Build a graph with nodes missing names — they still pass validation
    // because the validator only checks intent non-empty and graph structure.
    let mut node1 = define_node("X", "number", None);
    node1.metadata.name = None;
    let mut node2 = error_node("Y", vec![]);
    node2.metadata.name = None;

    let verified = build_verified_types_graph(vec![node1, node2]);
    let errors = emit_type_definitions(&verified).unwrap_err();

    assert_eq!(errors.len(), 2);
    assert!(matches!(errors[0], EmitError::DefineNodeMissingName { .. }));
    assert!(matches!(errors[1], EmitError::ErrorNodeMissingName { .. }));
}

// ── Python syntax verification ───────────────────────────────────────────────

#[test]
fn emit_generated_python_is_valid_syntax() {
    let verified = build_verified_types_graph(vec![
        define_node("WalletBalance", "number", Some("value >= 0")),
        define_node("PositiveAmount", "number", Some("value > 0")),
        define_node("UserId", "text", None),
        define_node("UserStatus", "text", None),
        describe_node(
            "User",
            vec![
                ("id", "UserId"),
                ("balance", "WalletBalance"),
                ("status", "UserStatus"),
            ],
        ),
        error_node(
            "InsufficientBalanceError",
            vec![
                ("current_balance", "WalletBalance"),
                ("requested_amount", "PositiveAmount"),
            ],
        ),
    ]);

    let output = emit_type_definitions(&verified).expect("emit should succeed");
    let code = &output.files[0].content;

    // Use Python's ast.parse to verify the emitted code is syntactically valid.
    let result = std::process::Command::new("python")
        .args(["-c", "import sys, ast; ast.parse(sys.stdin.read())"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(child) => child,
        Err(_) => {
            eprintln!("Python not available, skipping syntax check");
            return;
        }
    };

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(code.as_bytes())
        .expect("write to stdin");

    let output = child.wait_with_output().expect("wait for python");

    assert!(
        output.status.success(),
        "Generated Python has syntax errors:\n{}\n\nGenerated code:\n{}",
        String::from_utf8_lossy(&output.stderr),
        code
    );
}

// ── File ownership + __all__ tests ────────────────────────────────────────────

#[test]
fn types_py_has_generated_ownership() {
    let node = define_node("WalletBalance", "number", None);
    let verified = build_verified_types_graph(vec![node]);
    let output = emit_type_definitions(&verified).unwrap();
    assert_eq!(output.files[0].ownership, FileOwnership::Generated);
}

#[test]
fn types_py_has_all_list() {
    let node = define_node("WalletBalance", "number", None);
    let verified = build_verified_types_graph(vec![node]);
    let output = emit_type_definitions(&verified).unwrap();
    let content = &output.files[0].content;
    assert!(content.contains("__all__"), "types.py must contain __all__");
    assert!(
        content.contains("\"WalletBalance\""),
        "__all__ must include WalletBalance"
    );
}

#[test]
fn types_py_all_list_includes_all_kinds() {
    // Define + Describe + Error should all appear in __all__.
    let balance = define_node("Balance", "number", None);
    let transfer = describe_node("Transfer", vec![("amount", "number")]);
    let err = error_node("InsufficientFundsError", vec![]);
    let verified = build_verified_types_graph(vec![balance, transfer, err]);
    let output = emit_type_definitions(&verified).unwrap();
    let content = &output.files[0].content;
    assert!(content.contains("\"Balance\""));
    assert!(content.contains("\"Transfer\""));
    assert!(content.contains("\"InsufficientFundsError\""));
}
