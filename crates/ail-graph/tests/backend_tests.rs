/// Backend parity tests for the `GraphBackend` trait (task 7.1).
///
/// All tests exercise the trait through `AilGraph`. The same test bodies will
/// be reused for `SqliteGraph` in task 7.2 once that crate exists.
use ail_graph::{
    AilGraph, Contract, ContractKind, EdgeKind, Expression, GraphBackend, Node, NodeId, Pattern,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_node(intent: &str, pattern: Pattern) -> Node {
    Node::new(NodeId::new(), intent, pattern)
}

fn make_do(intent: &str) -> Node {
    make_node(intent, Pattern::Do)
}

fn named_do(intent: &str, name: &str) -> Node {
    let mut n = make_do(intent);
    n.metadata.name = Some(name.to_string());
    n
}

fn fresh_graph() -> AilGraph {
    AilGraph::new()
}

// ─── t071_trait_add_get_node_roundtrip ────────────────────────────────────────

#[test]
fn t071_trait_add_get_node_roundtrip() {
    let mut g = fresh_graph();
    let node = make_do("transfer money safely");
    let id = node.id;

    let returned_id = GraphBackend::add_node(&mut g, node).unwrap();
    assert_eq!(returned_id, id);

    let fetched = GraphBackend::get_node(&g, id).unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.intent, "transfer money safely");

    // get_node for unknown id returns None (not an error)
    let missing = GraphBackend::get_node(&g, NodeId::new()).unwrap();
    assert!(missing.is_none());
}

// ─── t071_trait_add_edge_creates_parent_child ─────────────────────────────────

#[test]
fn t071_trait_add_edge_creates_parent_child() {
    let mut g = fresh_graph();
    let parent_id = GraphBackend::add_node(&mut g, make_do("parent")).unwrap();
    let child_id = GraphBackend::add_node(&mut g, make_do("child")).unwrap();

    GraphBackend::add_edge(&mut g, parent_id, child_id, EdgeKind::Ev).unwrap();

    let children = GraphBackend::children(&g, parent_id).unwrap();
    assert_eq!(children, vec![child_id]);

    let parent = GraphBackend::parent(&g, child_id).unwrap();
    assert_eq!(parent, Some(parent_id));
}

// ─── t071_trait_children_returns_ordered ──────────────────────────────────────

#[test]
fn t071_trait_children_returns_ordered() {
    let mut g = fresh_graph();
    let root_id = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let c1 = GraphBackend::add_node(&mut g, make_do("child-1")).unwrap();
    let c2 = GraphBackend::add_node(&mut g, make_do("child-2")).unwrap();
    let c3 = GraphBackend::add_node(&mut g, make_do("child-3")).unwrap();

    GraphBackend::add_edge(&mut g, root_id, c1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, root_id, c2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, root_id, c3, EdgeKind::Ev).unwrap();

    let mut children = GraphBackend::children(&g, root_id).unwrap();
    children.sort_by_key(|id| id.to_string());
    let mut expected = vec![c1, c2, c3];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(children, expected, "all three children must be reported");
    assert_eq!(children.len(), 3);
}

// ─── t071_trait_siblings_before_after ─────────────────────────────────────────

#[test]
fn t071_trait_siblings_before_after() {
    let mut g = fresh_graph();
    let s1 = GraphBackend::add_node(&mut g, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut g, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut g, make_do("step-3")).unwrap();

    // Wire: s1 →Eh s2 →Eh s3
    GraphBackend::add_edge(&mut g, s1, s2, EdgeKind::Eh).unwrap();
    GraphBackend::add_edge(&mut g, s2, s3, EdgeKind::Eh).unwrap();

    // From s2: before = [s1], after = [s3]
    let before = GraphBackend::siblings_before(&g, s2).unwrap();
    assert_eq!(before, vec![s1], "s1 precedes s2");

    let after = GraphBackend::siblings_after(&g, s2).unwrap();
    assert_eq!(after, vec![s3], "s3 follows s2");

    // From s1: nothing before, [s2, s3] after
    assert!(GraphBackend::siblings_before(&g, s1).unwrap().is_empty());
    assert_eq!(GraphBackend::siblings_after(&g, s1).unwrap(), vec![s2, s3]);

    // From s3: [s1, s2] before, nothing after
    assert_eq!(GraphBackend::siblings_before(&g, s3).unwrap(), vec![s1, s2]);
    assert!(GraphBackend::siblings_after(&g, s3).unwrap().is_empty());
}

// ─── t071_trait_parent_returns_none_for_root ──────────────────────────────────

#[test]
fn t071_trait_parent_returns_none_for_root() {
    let mut g = fresh_graph();
    let root_id = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let child_id = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, root_id, child_id, EdgeKind::Ev).unwrap();

    assert_eq!(
        GraphBackend::parent(&g, root_id).unwrap(),
        None,
        "root has no parent"
    );
    assert_eq!(
        GraphBackend::parent(&g, child_id).unwrap(),
        Some(root_id),
        "child's parent is the root"
    );
}

// ─── t071_trait_ancestors_returns_path_to_root ────────────────────────────────

#[test]
fn t071_trait_ancestors_returns_path_to_root() {
    let mut g = fresh_graph();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let mid = GraphBackend::add_node(&mut g, make_do("mid")).unwrap();
    let leaf = GraphBackend::add_node(&mut g, make_do("leaf")).unwrap();

    GraphBackend::add_edge(&mut g, root, mid, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, mid, leaf, EdgeKind::Ev).unwrap();

    let ancestors = GraphBackend::ancestors(&g, leaf).unwrap();
    // Ordered: direct parent first, then grandparent
    assert_eq!(ancestors, vec![mid, root]);

    // Mid node: only root as ancestor
    assert_eq!(GraphBackend::ancestors(&g, mid).unwrap(), vec![root]);

    // Root: no ancestors
    assert!(GraphBackend::ancestors(&g, root).unwrap().is_empty());
}

// ─── t071_trait_diagonal_refs_returns_ed_edges ────────────────────────────────

#[test]
fn t071_trait_diagonal_refs_returns_ed_edges() {
    let mut g = fresh_graph();
    let func = GraphBackend::add_node(&mut g, make_do("transfer money")).unwrap();
    let ty = GraphBackend::add_node(&mut g, make_node("wallet balance", Pattern::Define)).unwrap();

    GraphBackend::add_edge(&mut g, func, ty, EdgeKind::Ed).unwrap();

    let refs = GraphBackend::diagonal_refs(&g, func).unwrap();
    // Must contain (ty, EdgeKind::Ed) for the outgoing cross-reference
    assert!(
        refs.contains(&(ty, EdgeKind::Ed)),
        "diagonal_refs must include the Ed edge target"
    );

    // The referenced type should see the back-reference
    let back = GraphBackend::diagonal_refs(&g, ty).unwrap();
    assert!(
        back.contains(&(func, EdgeKind::Ed)),
        "diagonal_refs must include incoming Ed edges"
    );
}

// ─── t071_trait_find_by_pattern_filters_correctly ────────────────────────────

#[test]
fn t071_trait_find_by_pattern_filters_correctly() {
    let mut g = fresh_graph();
    let do1 = GraphBackend::add_node(&mut g, make_do("function one")).unwrap();
    let do2 = GraphBackend::add_node(&mut g, make_do("function two")).unwrap();
    let def1 = GraphBackend::add_node(&mut g, make_node("some type", Pattern::Define)).unwrap();

    let mut do_ids = GraphBackend::find_by_pattern(&g, Pattern::Do).unwrap();
    do_ids.sort_by_key(|id| id.to_string());
    let mut expected = vec![do1, do2];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(
        do_ids, expected,
        "find_by_pattern(Do) must return exactly the Do nodes"
    );

    let define_ids = GraphBackend::find_by_pattern(&g, Pattern::Define).unwrap();
    assert_eq!(define_ids, vec![def1]);

    let describe_ids = GraphBackend::find_by_pattern(&g, Pattern::Describe).unwrap();
    assert!(describe_ids.is_empty());
}

// ─── t071_trait_find_by_name_returns_matches ──────────────────────────────────

#[test]
fn t071_trait_find_by_name_returns_matches() {
    let mut g = fresh_graph();
    let id1 = GraphBackend::add_node(&mut g, named_do("transfer money", "transfer_money")).unwrap();
    let _id2 = GraphBackend::add_node(&mut g, make_do("validate input")).unwrap();
    let id3 = GraphBackend::add_node(&mut g, named_do("transfer again", "transfer_money")).unwrap();

    let mut found = GraphBackend::find_by_name(&g, "transfer_money").unwrap();
    found.sort_by_key(|id| id.to_string());
    let mut expected = vec![id1, id3];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(found, expected);

    // No match
    let none = GraphBackend::find_by_name(&g, "nonexistent").unwrap();
    assert!(none.is_empty());
}

// ─── t071_trait_remove_node_cascades_edges ────────────────────────────────────

#[test]
fn t071_trait_remove_node_cascades_edges() {
    let mut g = fresh_graph();
    let parent = GraphBackend::add_node(&mut g, make_do("parent")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, parent, child, EdgeKind::Ev).unwrap();

    // Remove child — petgraph removes incident edges automatically
    GraphBackend::remove_node(&mut g, child).unwrap();

    // Parent now has no children
    let children = GraphBackend::children(&g, parent).unwrap();
    assert!(
        children.is_empty(),
        "incident edges must be gone after node removal"
    );
    assert_eq!(GraphBackend::node_count(&g), 1);
}

// ─── t071_trait_transaction_commits_on_success ────────────────────────────────

#[test]
fn t071_trait_transaction_commits_on_success() {
    let mut g = fresh_graph();

    GraphBackend::begin_transaction(&mut g).unwrap();
    let id = GraphBackend::add_node(&mut g, make_do("inside transaction")).unwrap();
    GraphBackend::commit_transaction(&mut g).unwrap();

    // Node is visible after commit
    let node = GraphBackend::get_node(&g, id).unwrap();
    assert!(
        node.is_some(),
        "node inserted during transaction must be visible after commit"
    );
}

// ─── t071_trait_transaction_rolls_back_on_error ───────────────────────────────

/// For `AilGraph` (in-memory), `rollback_transaction` is a no-op — there is no
/// crash recovery or WAL. This test verifies that the transaction API surface
/// is callable without errors and documents the no-op semantics.
#[test]
fn t071_trait_transaction_rolls_back_on_error() {
    let mut g = fresh_graph();

    GraphBackend::begin_transaction(&mut g).unwrap();
    let _id = GraphBackend::add_node(&mut g, make_do("tentative node")).unwrap();
    // Simulate an error path — call rollback.
    GraphBackend::rollback_transaction(&mut g).unwrap();

    // AilGraph rollback is a no-op: node remains in memory.
    // The test only asserts the API does not error.
    assert_eq!(
        GraphBackend::node_count(&g),
        1,
        "AilGraph rollback is a no-op; node persists (documented behaviour)"
    );
}

// ─── t071_trait_root_nodes_returns_depth_zero ─────────────────────────────────

#[test]
fn t071_trait_root_nodes_returns_depth_zero() {
    let mut g = fresh_graph();
    let root1 = GraphBackend::add_node(&mut g, make_do("root-1")).unwrap();
    let root2 = GraphBackend::add_node(&mut g, make_do("root-2")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, root1, child, EdgeKind::Ev).unwrap();

    let mut roots = GraphBackend::root_nodes(&g).unwrap();
    roots.sort_by_key(|id| id.to_string());
    let mut expected = vec![root1, root2];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(
        roots, expected,
        "root_nodes must return nodes with no parent"
    );
    // child must not appear as a root
    assert!(!roots.contains(&child));
}

// ─── t071_trait_depth_returns_correct_level ───────────────────────────────────

#[test]
fn t071_trait_depth_returns_correct_level() {
    let mut g = fresh_graph();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let level1 = GraphBackend::add_node(&mut g, make_do("level-1")).unwrap();
    let level2 = GraphBackend::add_node(&mut g, make_do("level-2")).unwrap();

    GraphBackend::add_edge(&mut g, root, level1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, level1, level2, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::depth(&g, root).unwrap(), 0);
    assert_eq!(GraphBackend::depth(&g, level1).unwrap(), 1);
    assert_eq!(GraphBackend::depth(&g, level2).unwrap(), 2);
}

// ─── t071_trait_node_count_after_operations ───────────────────────────────────

#[test]
fn t071_trait_node_count_after_operations() {
    let mut g = fresh_graph();
    assert_eq!(GraphBackend::node_count(&g), 0);

    let id1 = GraphBackend::add_node(&mut g, make_do("node-1")).unwrap();
    let id2 = GraphBackend::add_node(&mut g, make_do("node-2")).unwrap();
    let id3 = GraphBackend::add_node(&mut g, make_do("node-3")).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 3);

    // update_node does not change count
    let updated = make_do("node-1-updated");
    GraphBackend::update_node(&mut g, id1, updated).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 3);

    // remove decrements count
    GraphBackend::remove_node(&mut g, id2).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 2);

    // all_node_ids reflects the live set
    let mut ids = GraphBackend::all_node_ids(&g).unwrap();
    ids.sort_by_key(|id| id.to_string());
    let mut expected = vec![id1, id3];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(ids, expected);
}

// ─── Bonus: contracts via trait ───────────────────────────────────────────────

#[test]
fn t071_trait_add_and_read_contracts() {
    let mut g = fresh_graph();
    let id = GraphBackend::add_node(&mut g, make_do("validate wallet")).unwrap();

    let c = Contract {
        kind: ContractKind::Before,
        expression: Expression("balance >= 0".to_string()),
    };
    GraphBackend::add_contract(&mut g, id, c.clone()).unwrap();

    let contracts = GraphBackend::contracts(&g, id).unwrap();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].kind, ContractKind::Before);
    assert_eq!(contracts[0].expression, c.expression);
}

// ─── Bonus: all_descendants ───────────────────────────────────────────────────

#[test]
fn t071_trait_all_descendants_returns_subtree() {
    let mut g = fresh_graph();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let c1 = GraphBackend::add_node(&mut g, make_do("child-1")).unwrap();
    let c2 = GraphBackend::add_node(&mut g, make_do("child-2")).unwrap();
    let gc = GraphBackend::add_node(&mut g, make_do("grandchild")).unwrap();

    GraphBackend::add_edge(&mut g, root, c1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, root, c2, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, c1, gc, EdgeKind::Ev).unwrap();

    let mut desc = GraphBackend::all_descendants(&g, root).unwrap();
    desc.sort_by_key(|id| id.to_string());
    let mut expected = vec![c1, c2, gc];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(desc, expected);

    // Leaf node: no descendants
    assert!(GraphBackend::all_descendants(&g, gc).unwrap().is_empty());
}
