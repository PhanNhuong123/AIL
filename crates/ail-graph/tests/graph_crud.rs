use ail_graph::{AilGraph, EdgeKind, GraphError, Node, NodeId, Pattern};

fn make_node(intent: &str) -> Node {
    Node::new(NodeId::new(), intent, Pattern::Do)
}

// ─── node count ────────────────────────────────────────────────────────────

#[test]
fn crud_new_graph_is_empty() {
    let graph = AilGraph::new();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.root_id(), None);
}

// ─── add_node ──────────────────────────────────────────────────────────────

#[test]
fn crud_add_node_returns_its_id() {
    let mut graph = AilGraph::new();
    let node = make_node("transfer money");
    let expected_id = node.id;
    let returned_id = graph.add_node(node).unwrap();
    assert_eq!(returned_id, expected_id);
    assert_eq!(graph.node_count(), 1);
}

#[test]
fn crud_add_duplicate_node_id_returns_error() {
    let mut graph = AilGraph::new();
    let node = make_node("transfer money");
    let duplicate = node.clone();
    graph.add_node(node).unwrap();
    let result = graph.add_node(duplicate);
    assert!(
        matches!(result, Err(GraphError::DuplicateNodeId(_))),
        "expected DuplicateNodeId, got {result:?}"
    );
    // original node is still present
    assert_eq!(graph.node_count(), 1);
}

// ─── remove_node ───────────────────────────────────────────────────────────

#[test]
fn crud_remove_existing_node_succeeds() {
    let mut graph = AilGraph::new();
    let id = graph.add_node(make_node("deduct balance")).unwrap();
    let removed = graph.remove_node(id).unwrap();
    assert_eq!(removed.id, id);
    assert_eq!(graph.node_count(), 0);
}

#[test]
fn crud_remove_missing_node_returns_error() {
    let mut graph = AilGraph::new();
    let phantom_id = NodeId::new();
    let result = graph.remove_node(phantom_id);
    assert!(
        matches!(result, Err(GraphError::NodeNotFound(_))),
        "expected NodeNotFound, got {result:?}"
    );
}

// ─── get_node ──────────────────────────────────────────────────────────────

#[test]
fn crud_get_existing_node_returns_correct_node() {
    let mut graph = AilGraph::new();
    let node = make_node("validate wallet");
    let id = node.id;
    graph.add_node(node).unwrap();
    let fetched = graph.get_node(id).unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.intent, "validate wallet");
}

#[test]
fn crud_get_missing_node_returns_error() {
    let graph = AilGraph::new();
    let result = graph.get_node(NodeId::new());
    assert!(
        matches!(result, Err(GraphError::NodeNotFound(_))),
        "expected NodeNotFound, got {result:?}"
    );
}

// ─── add_edge ──────────────────────────────────────────────────────────────

#[test]
fn crud_add_edge_between_two_nodes_succeeds() {
    let mut graph = AilGraph::new();
    let a = graph.add_node(make_node("parent")).unwrap();
    let b = graph.add_node(make_node("child")).unwrap();
    graph.add_edge(a, b, EdgeKind::Ev).unwrap();
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn crud_add_edge_with_missing_source_returns_error() {
    let mut graph = AilGraph::new();
    let b = graph.add_node(make_node("child")).unwrap();
    let result = graph.add_edge(NodeId::new(), b, EdgeKind::Ev);
    assert!(
        matches!(result, Err(GraphError::NodeNotFound(_))),
        "expected NodeNotFound, got {result:?}"
    );
}

#[test]
fn crud_add_edge_with_missing_target_returns_error() {
    let mut graph = AilGraph::new();
    let a = graph.add_node(make_node("parent")).unwrap();
    let result = graph.add_edge(a, NodeId::new(), EdgeKind::Ev);
    assert!(
        matches!(result, Err(GraphError::NodeNotFound(_))),
        "expected NodeNotFound, got {result:?}"
    );
}

// ─── remove_edge ───────────────────────────────────────────────────────────

#[test]
fn crud_remove_edge_succeeds() {
    let mut graph = AilGraph::new();
    let a = graph.add_node(make_node("parent")).unwrap();
    let b = graph.add_node(make_node("child")).unwrap();
    let edge_id = graph.add_edge(a, b, EdgeKind::Ev).unwrap();
    graph.remove_edge(edge_id).unwrap();
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn crud_remove_same_edge_twice_returns_error() {
    let mut graph = AilGraph::new();
    let a = graph.add_node(make_node("parent")).unwrap();
    let b = graph.add_node(make_node("child")).unwrap();
    let edge_id = graph.add_edge(a, b, EdgeKind::Ev).unwrap();
    graph.remove_edge(edge_id).unwrap();
    let result = graph.remove_edge(edge_id);
    assert!(
        matches!(result, Err(GraphError::EdgeNotFound(_))),
        "expected EdgeNotFound, got {result:?}"
    );
}

// ─── remove_node clears root ───────────────────────────────────────────────

#[test]
fn crud_remove_root_node_clears_root_id() {
    let mut graph = AilGraph::new();
    let id = graph.add_node(make_node("root")).unwrap();
    graph.set_root(id).unwrap();
    assert_eq!(graph.root_id(), Some(id));
    graph.remove_node(id).unwrap();
    assert_eq!(graph.root_id(), None, "root_id must be cleared after root is removed");
}

// ─── set_root ──────────────────────────────────────────────────────────────

#[test]
fn crud_set_root_records_root_id() {
    let mut graph = AilGraph::new();
    let id = graph.add_node(make_node("root")).unwrap();
    graph.set_root(id).unwrap();
    assert_eq!(graph.root_id(), Some(id));
}

#[test]
fn crud_set_root_with_missing_node_returns_error() {
    let mut graph = AilGraph::new();
    let result = graph.set_root(NodeId::new());
    assert!(
        matches!(result, Err(GraphError::NodeNotFound(_))),
        "expected NodeNotFound, got {result:?}"
    );
}
