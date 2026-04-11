use ail_graph::{AilGraph, EdgeKind, Node, NodeId, Pattern};

fn make_node(intent: &str) -> Node {
    Node::new(NodeId::new(), intent, Pattern::Do)
}

/// Build a small graph: root → child_a → child_b (Ev tree)
/// with child_a –Eh→ child_b.
fn build_linear_tree() -> (AilGraph, NodeId, NodeId, NodeId) {
    let mut graph = AilGraph::new();
    let root = graph.add_node(make_node("root")).unwrap();
    let child_a = graph.add_node(make_node("step-a")).unwrap();
    let child_b = graph.add_node(make_node("step-b")).unwrap();

    graph.add_edge(root, child_a, EdgeKind::Ev).unwrap();
    graph.add_edge(root, child_b, EdgeKind::Ev).unwrap();
    graph.add_edge(child_a, child_b, EdgeKind::Eh).unwrap();
    graph.set_root(root).unwrap();

    (graph, root, child_a, child_b)
}

// ─── children_of ───────────────────────────────────────────────────────────

#[test]
fn navigate_children_of_ev_edges_returns_targets() {
    let (graph, root, child_a, child_b) = build_linear_tree();
    let mut children = graph.children_of(root).unwrap();
    children.sort_by_key(|id| id.to_string());

    let mut expected = vec![child_a, child_b];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(children, expected);
}

#[test]
fn navigate_leaf_node_has_no_children() {
    let (graph, _, _, child_b) = build_linear_tree();
    // child_b has no outgoing Ev edges
    let children = graph.children_of(child_b).unwrap();
    assert!(children.is_empty());
}

// ─── parent_of ─────────────────────────────────────────────────────────────

#[test]
fn navigate_parent_of_returns_source_of_ev_edge() {
    let (graph, root, child_a, _) = build_linear_tree();
    let parent = graph.parent_of(child_a).unwrap();
    assert_eq!(parent, Some(root));
}

#[test]
fn navigate_root_has_no_parent() {
    let (graph, root, _, _) = build_linear_tree();
    let parent = graph.parent_of(root).unwrap();
    assert_eq!(parent, None);
}

// ─── next_sibling_of ───────────────────────────────────────────────────────

#[test]
fn navigate_next_sibling_follows_eh_edge() {
    let (graph, _, child_a, child_b) = build_linear_tree();
    let next = graph.next_sibling_of(child_a).unwrap();
    assert_eq!(next, Some(child_b));
}

#[test]
fn navigate_last_sibling_has_no_next() {
    let (graph, _, _, child_b) = build_linear_tree();
    let next = graph.next_sibling_of(child_b).unwrap();
    assert_eq!(next, None);
}

// ─── prev_sibling_of ───────────────────────────────────────────────────────

#[test]
fn navigate_prev_sibling_follows_incoming_eh_edge() {
    let (graph, _, child_a, child_b) = build_linear_tree();
    let prev = graph.prev_sibling_of(child_b).unwrap();
    assert_eq!(prev, Some(child_a));
}

#[test]
fn navigate_first_sibling_has_no_prev() {
    let (graph, _, child_a, _) = build_linear_tree();
    let prev = graph.prev_sibling_of(child_a).unwrap();
    assert_eq!(prev, None);
}

// ─── diagonal_refs_of ──────────────────────────────────────────────────────

#[test]
fn navigate_diagonal_refs_includes_both_directions() {
    let mut graph = AilGraph::new();
    let src = graph.add_node(make_node("function")).unwrap();
    let type_a = graph.add_node(make_node("TypeA")).unwrap();
    let type_b = graph.add_node(make_node("TypeB")).unwrap();

    // outgoing Ed edge from src → type_a
    graph.add_edge(src, type_a, EdgeKind::Ed).unwrap();
    // incoming Ed edge from type_b → src
    graph.add_edge(type_b, src, EdgeKind::Ed).unwrap();

    let mut refs = graph.diagonal_refs_of(src).unwrap();
    refs.sort_by_key(|id| id.to_string());

    let mut expected = vec![type_a, type_b];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(refs, expected);
}

#[test]
fn navigate_node_with_no_diagonal_edges_returns_empty() {
    let (graph, root, _, _) = build_linear_tree();
    // root has no Ed edges
    let refs = graph.diagonal_refs_of(root).unwrap();
    assert!(refs.is_empty());
}
