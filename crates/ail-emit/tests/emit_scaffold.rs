use ail_contract::verify;
use ail_emit::{emit_scaffold_files, FileOwnership};
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Pattern,
};
use ail_types::type_check;

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Build a minimal verified graph (root Describe only — no type or function nodes).
fn build_minimal_verified() -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();
    let root = Node {
        id: NodeId::new(),
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();
    let valid = validate_graph(graph).unwrap();
    let typed = type_check(valid, &[]).unwrap();
    verify(typed).unwrap()
}

/// Build a verified graph with a single Do function node (needed to confirm scaffold is
/// graph-independent — same file is emitted regardless of graph content).
fn build_verified_with_do() -> ail_contract::VerifiedGraph {
    let mut graph = AilGraph::new();
    let root = Node {
        id: NodeId::new(),
        intent: "root".to_owned(),
        pattern: Pattern::Describe,
        children: None,
        expression: None,
        contracts: vec![],
        metadata: NodeMetadata::default(),
    };
    let root_id = root.id;
    graph.add_node(root).unwrap();
    graph.set_root(root_id).unwrap();

    let mut do_node = Node {
        id: NodeId::new(),
        intent: "transfer".to_owned(),
        pattern: Pattern::Do,
        children: None,
        expression: None,
        contracts: vec![
            Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            },
            Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            },
        ],
        metadata: NodeMetadata::default(),
    };
    do_node.metadata.name = Some("transfer".to_owned());
    do_node.metadata.return_type = Some("number".to_owned());

    let do_id = do_node.id;
    graph.add_node(do_node).unwrap();
    graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

    let valid = validate_graph(graph).unwrap();
    let typed = type_check(valid, &[]).unwrap();
    verify(typed).unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn scaffold_produces_exactly_one_file() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert_eq!(output.files.len(), 1);
}

#[test]
fn scaffold_file_has_scaffolded_ownership() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert_eq!(output.files[0].ownership, FileOwnership::Scaffolded);
}

#[test]
fn scaffold_path_is_scaffolded_init() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert_eq!(output.files[0].path, "scaffolded/__init__.py");
}

#[test]
fn scaffold_content_has_future_annotations() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert!(output.files[0].content.contains("from __future__ import annotations"));
}

#[test]
fn scaffold_content_imports_generated_types() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert!(output.files[0].content.contains("from generated.types import *"));
}

#[test]
fn scaffold_content_imports_generated_functions() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert!(output.files[0].content.contains("from generated.functions import *"));
}

#[test]
fn scaffold_content_uses_absolute_imports() {
    // The scaffold uses absolute imports (no leading dot), so it works when the
    // output root is on PYTHONPATH. Relative imports would require scaffolded/ to
    // be inside the generated package, which contradicts the peer-directory layout.
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    // Absolute: "from generated." (no ".." before "generated")
    assert!(output.files[0].content.contains("from generated."));
    // Not a relative import (would start with "from .")
    assert!(!output.files[0].content.contains("from .generated."));
}

#[test]
fn scaffold_is_graph_independent() {
    // Scaffold should be identical whether the graph has no nodes or many nodes.
    let empty = emit_scaffold_files(&build_minimal_verified());
    let with_do = emit_scaffold_files(&build_verified_with_do());
    assert_eq!(empty.files[0].content, with_do.files[0].content);
}

#[test]
fn scaffold_mentions_never_overwritten_in_comment() {
    let verified = build_minimal_verified();
    let output = emit_scaffold_files(&verified);
    assert!(output.files[0].content.contains("never overwritten"));
}
