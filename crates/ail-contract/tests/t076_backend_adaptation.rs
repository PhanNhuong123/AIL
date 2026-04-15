//! Task 7.6 — Downstream Crate Adaptation: backend trait integration tests.
//!
//! Verifies that `ValidGraph::graph()`, `TypedGraph::graph()`, and
//! `VerifiedGraph::graph()` all return `&dyn GraphBackend` with correct
//! node counts and that `all_nodes_vec()` is accessible through the trait.
//!
//! The three wrappers expose a backend-agnostic interface. These tests confirm
//! that the trait API is reachable through each pipeline stage using the
//! in-memory `AilGraph` backend (which implements `GraphBackend`).

use ail_contract::verify;
use ail_graph::{
    validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
    NodeMetadata, Param, Pattern,
};
use ail_types::type_check;

// ── Fixture helpers ───────────────────────────────────────────────────────────

/// Build a minimal wallet-like graph:
/// - Describe root
/// - Define node for `WalletId` (base: `text`)
/// - Do node `transfer money` with before + after contracts
///
/// Returns `(graph, expected_node_count)`.
fn build_wallet_graph() -> (AilGraph, usize) {
    let mut graph = AilGraph::new();

    // Structural root
    let root_id = {
        let n = Node {
            id: NodeId::new(),
            intent: "wallet domain".to_owned(),
            pattern: Pattern::Describe,
            children: Some(vec![]),
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        let id = n.id;
        graph.add_node(n).unwrap();
        graph.set_root(id).unwrap();
        id
    };

    // Define node — needed so the type-checker can resolve `WalletId`.
    let define_id = {
        let mut n = Node {
            id: NodeId::new(),
            intent: "wallet identifier".to_owned(),
            pattern: Pattern::Define,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        n.metadata.name = Some("WalletId".to_owned());
        n.metadata.base_type = Some("text".to_owned());
        let id = n.id;
        graph.add_node(n).unwrap();
        graph.add_edge(root_id, id, EdgeKind::Ev).unwrap();
        graph
            .get_node_mut(root_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(id);
        id
    };
    let _ = define_id;

    // Do node with required Before + After contracts.
    let _do_id = {
        let mut n = Node {
            id: NodeId::new(),
            intent: "transfer money".to_owned(),
            pattern: Pattern::Do,
            children: None,
            expression: None,
            contracts: vec![
                Contract {
                    kind: ContractKind::Before,
                    expression: Expression("amount > 0".to_owned()),
                },
                Contract {
                    kind: ContractKind::After,
                    expression: Expression("result == true".to_owned()),
                },
            ],
            metadata: NodeMetadata::default(),
        };
        n.metadata.name = Some("transfer money".to_owned());
        n.metadata.params = vec![
            Param {
                name: "sender_id".to_owned(),
                type_ref: "WalletId".to_owned(),
            },
            Param {
                name: "amount".to_owned(),
                type_ref: "number".to_owned(),
            },
        ];
        n.metadata.return_type = Some("boolean".to_owned());
        let id = n.id;
        graph.add_node(n).unwrap();
        graph.add_edge(root_id, id, EdgeKind::Ev).unwrap();
        graph
            .get_node_mut(root_id)
            .unwrap()
            .children
            .as_mut()
            .unwrap()
            .push(id);
        id
    };

    let count = graph.node_count();
    (graph, count)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// `ValidGraph::graph()` returns `&dyn GraphBackend`.
///
/// Asserts that `node_count()` and `all_nodes_vec()` are accessible through the
/// trait object and return consistent values.
#[test]
fn t076_valid_graph_wraps_sqlite_backend() {
    let (graph, original_count) = build_wallet_graph();
    let valid = validate_graph(graph).expect("wallet graph must validate");

    let backend = valid.graph(); // &dyn GraphBackend
    assert_eq!(
        backend.node_count(),
        original_count,
        "node_count() through &dyn GraphBackend must match original AilGraph"
    );

    let nodes = backend.all_nodes_vec();
    assert_eq!(
        nodes.len(),
        original_count,
        "all_nodes_vec() through &dyn GraphBackend must return all nodes"
    );

    // Confirm trait call actually performed vtable dispatch: all patterns are known.
    let patterns: Vec<_> = nodes.iter().map(|n| n.pattern.clone()).collect();
    assert!(
        patterns.contains(&Pattern::Describe),
        "root Describe node must be reachable via all_nodes_vec()"
    );
    assert!(
        patterns.contains(&Pattern::Do),
        "Do node must be reachable via all_nodes_vec()"
    );
}

/// `TypedGraph::graph()` returns `&dyn GraphBackend` with matching node count.
#[test]
fn t076_typed_graph_wraps_sqlite_backend() {
    let (graph, original_count) = build_wallet_graph();
    let valid = validate_graph(graph).expect("wallet graph must validate");
    let typed = type_check(valid, &[]).expect("wallet graph must type-check");

    let backend = typed.graph(); // &dyn GraphBackend
    assert_eq!(
        backend.node_count(),
        original_count,
        "TypedGraph::graph().node_count() must match original node count"
    );
    assert_eq!(
        backend.all_nodes_vec().len(),
        original_count,
        "TypedGraph::graph().all_nodes_vec() must return all nodes"
    );
}

/// `VerifiedGraph::graph()` returns `&dyn GraphBackend` with matching node count.
#[test]
fn t076_verified_graph_wraps_sqlite_backend() {
    let (graph, original_count) = build_wallet_graph();
    let valid = validate_graph(graph).expect("wallet graph must validate");
    let typed = type_check(valid, &[]).expect("wallet graph must type-check");
    let verified = verify(typed).expect("wallet graph must verify");

    let backend = verified.graph(); // &dyn GraphBackend
    assert_eq!(
        backend.node_count(),
        original_count,
        "VerifiedGraph::graph().node_count() must match original node count"
    );

    let nodes = backend.all_nodes_vec();
    assert_eq!(
        nodes.len(),
        original_count,
        "VerifiedGraph::graph().all_nodes_vec() must return all nodes"
    );

    // Confirm the Do node with contracts is accessible through the trait.
    let do_node = nodes.into_iter().find(|n| n.pattern == Pattern::Do);
    assert!(
        do_node.is_some(),
        "Do node must be reachable via VerifiedGraph::graph()"
    );
    let do_node = do_node.unwrap();
    assert_eq!(
        do_node.contracts.len(),
        2,
        "before + after contracts must be preserved"
    );
}
