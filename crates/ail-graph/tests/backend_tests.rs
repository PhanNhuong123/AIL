/// Backend parity tests for the `GraphBackend` trait (task 7.1).
///
/// All tests exercise the trait interface through `AilGraph`. The same test
/// bodies will drive `SqliteGraph` in task 7.2 once that crate exists.
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

fn fresh() -> AilGraph {
    AilGraph::new()
}

// ─── t071_trait_add_get_node_roundtrip ────────────────────────────────────────

#[test]
fn t071_trait_add_get_node_roundtrip() {
    let mut g = fresh();
    let node = make_do("transfer money safely");
    let id = node.id;

    let returned = GraphBackend::add_node(&mut g, node).unwrap();
    assert_eq!(returned, id);

    let fetched = GraphBackend::get_node(&g, id).unwrap().unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.intent, "transfer money safely");

    // Unknown id → None, not an error.
    assert!(GraphBackend::get_node(&g, NodeId::new()).unwrap().is_none());
}

// ─── t071_trait_update_node ───────────────────────────────────────────────────

#[test]
fn t071_trait_update_node() {
    let mut g = fresh();
    let id = GraphBackend::add_node(&mut g, make_do("original")).unwrap();

    let replacement = make_do("updated intent");
    GraphBackend::update_node(&mut g, id, replacement).unwrap();

    let fetched = GraphBackend::get_node(&g, id).unwrap().unwrap();
    assert_eq!(fetched.intent, "updated intent");
    assert_eq!(GraphBackend::node_count(&g), 1, "update must not change count");
}

// ─── t071_trait_add_edge_creates_parent_child ─────────────────────────────────

#[test]
fn t071_trait_add_edge_creates_parent_child() {
    let mut g = fresh();
    let parent = GraphBackend::add_node(&mut g, make_do("parent")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();

    GraphBackend::add_edge(&mut g, parent, child, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::children(&g, parent).unwrap(), vec![child]);
    assert_eq!(GraphBackend::parent(&g, child).unwrap(), Some(parent));
}

// ─── t071_trait_remove_edge_by_kind ──────────────────────────────────────────

#[test]
fn t071_trait_remove_edge_by_kind() {
    let mut g = fresh();
    let a = GraphBackend::add_node(&mut g, make_do("a")).unwrap();
    let b = GraphBackend::add_node(&mut g, make_do("b")).unwrap();

    GraphBackend::add_edge(&mut g, a, b, EdgeKind::Ed).unwrap();
    assert!(!GraphBackend::diagonal_refs(&g, a).unwrap().is_empty());

    GraphBackend::remove_edge_by_kind(&mut g, a, b, EdgeKind::Ed).unwrap();
    assert!(GraphBackend::diagonal_refs(&g, a).unwrap().is_empty());

    // Removing a non-existent edge must error.
    let err = GraphBackend::remove_edge_by_kind(&mut g, a, b, EdgeKind::Ed);
    assert!(err.is_err(), "removing missing edge must return Err");
}

// ─── t071_trait_children_returns_all_ev_targets ───────────────────────────────

#[test]
fn t071_trait_children_returns_all_ev_targets() {
    let mut g = fresh();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let c1 = GraphBackend::add_node(&mut g, make_do("child-1")).unwrap();
    let c2 = GraphBackend::add_node(&mut g, make_do("child-2")).unwrap();
    let c3 = GraphBackend::add_node(&mut g, make_do("child-3")).unwrap();

    for c in [c1, c2, c3] {
        GraphBackend::add_edge(&mut g, root, c, EdgeKind::Ev).unwrap();
    }

    let mut children = GraphBackend::children(&g, root).unwrap();
    children.sort_by_key(|id| id.to_string());
    let mut expected = vec![c1, c2, c3];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(children, expected);
}

// ─── t071_trait_siblings_before_after ─────────────────────────────────────────

#[test]
fn t071_trait_siblings_before_after() {
    let mut g = fresh();
    let s1 = GraphBackend::add_node(&mut g, make_do("step-1")).unwrap();
    let s2 = GraphBackend::add_node(&mut g, make_do("step-2")).unwrap();
    let s3 = GraphBackend::add_node(&mut g, make_do("step-3")).unwrap();

    // Chain: s1 →Eh s2 →Eh s3
    GraphBackend::add_edge(&mut g, s1, s2, EdgeKind::Eh).unwrap();
    GraphBackend::add_edge(&mut g, s2, s3, EdgeKind::Eh).unwrap();

    // From s2: one before, one after.
    assert_eq!(GraphBackend::siblings_before(&g, s2).unwrap(), vec![s1]);
    assert_eq!(GraphBackend::siblings_after(&g, s2).unwrap(), vec![s3]);

    // From s1: nothing before; s2 and s3 after (earliest-first).
    assert!(GraphBackend::siblings_before(&g, s1).unwrap().is_empty());
    assert_eq!(GraphBackend::siblings_after(&g, s1).unwrap(), vec![s2, s3]);

    // From s3: s1 and s2 before (earliest-first); nothing after.
    assert_eq!(GraphBackend::siblings_before(&g, s3).unwrap(), vec![s1, s2]);
    assert!(GraphBackend::siblings_after(&g, s3).unwrap().is_empty());
}

// ─── t071_trait_parent_returns_none_for_root ──────────────────────────────────

#[test]
fn t071_trait_parent_returns_none_for_root() {
    let mut g = fresh();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, root, child, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::parent(&g, root).unwrap(), None);
    assert_eq!(GraphBackend::parent(&g, child).unwrap(), Some(root));
}

// ─── t071_trait_ancestors_returns_path_to_root ────────────────────────────────

#[test]
fn t071_trait_ancestors_returns_path_to_root() {
    let mut g = fresh();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let mid = GraphBackend::add_node(&mut g, make_do("mid")).unwrap();
    let leaf = GraphBackend::add_node(&mut g, make_do("leaf")).unwrap();

    GraphBackend::add_edge(&mut g, root, mid, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, mid, leaf, EdgeKind::Ev).unwrap();

    // Ordered direct-parent first, root last.
    assert_eq!(GraphBackend::ancestors(&g, leaf).unwrap(), vec![mid, root]);
    assert_eq!(GraphBackend::ancestors(&g, mid).unwrap(), vec![root]);
    assert!(GraphBackend::ancestors(&g, root).unwrap().is_empty());
}

// ─── t071_trait_all_descendants_returns_subtree ───────────────────────────────

#[test]
fn t071_trait_all_descendants_returns_subtree() {
    let mut g = fresh();
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

    // Leaf node has no descendants.
    assert!(GraphBackend::all_descendants(&g, gc).unwrap().is_empty());
}

// ─── t071_trait_diagonal_refs_returns_ed_edges ────────────────────────────────

#[test]
fn t071_trait_diagonal_refs_returns_ed_edges() {
    let mut g = fresh();
    let func = GraphBackend::add_node(&mut g, make_do("transfer money")).unwrap();
    let ty = GraphBackend::add_node(&mut g, make_node("wallet balance", Pattern::Define)).unwrap();

    GraphBackend::add_edge(&mut g, func, ty, EdgeKind::Ed).unwrap();

    // Source sees the outgoing cross-reference.
    let from_func = GraphBackend::diagonal_refs(&g, func).unwrap();
    assert!(
        from_func.contains(&(ty, EdgeKind::Ed)),
        "diagonal_refs must include outgoing Ed target"
    );

    // Target sees the incoming back-reference.
    let from_ty = GraphBackend::diagonal_refs(&g, ty).unwrap();
    assert!(
        from_ty.contains(&(func, EdgeKind::Ed)),
        "diagonal_refs must include incoming Ed source"
    );
}

// ─── t071_trait_find_by_pattern_filters_correctly ─────────────────────────────

#[test]
fn t071_trait_find_by_pattern_filters_correctly() {
    let mut g = fresh();
    let do1 = GraphBackend::add_node(&mut g, make_do("fn one")).unwrap();
    let do2 = GraphBackend::add_node(&mut g, make_do("fn two")).unwrap();
    let def = GraphBackend::add_node(&mut g, make_node("some type", Pattern::Define)).unwrap();

    let mut do_ids = GraphBackend::find_by_pattern(&g, Pattern::Do).unwrap();
    do_ids.sort_by_key(|id| id.to_string());
    let mut expected = vec![do1, do2];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(do_ids, expected);

    assert_eq!(GraphBackend::find_by_pattern(&g, Pattern::Define).unwrap(), vec![def]);
    assert!(GraphBackend::find_by_pattern(&g, Pattern::Describe).unwrap().is_empty());
}

// ─── t071_trait_find_by_name_returns_matches ──────────────────────────────────

#[test]
fn t071_trait_find_by_name_returns_matches() {
    let mut g = fresh();
    let id1 = GraphBackend::add_node(&mut g, named_do("transfer money", "transfer_money")).unwrap();
    let _id2 = GraphBackend::add_node(&mut g, make_do("validate input")).unwrap();
    let id3 = GraphBackend::add_node(&mut g, named_do("transfer again", "transfer_money")).unwrap();

    let mut found = GraphBackend::find_by_name(&g, "transfer_money").unwrap();
    found.sort_by_key(|id| id.to_string());
    let mut expected = vec![id1, id3];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(found, expected);

    assert!(GraphBackend::find_by_name(&g, "nonexistent").unwrap().is_empty());
}

// ─── t071_trait_root_nodes_excludes_children ──────────────────────────────────

#[test]
fn t071_trait_root_nodes_excludes_children() {
    let mut g = fresh();
    let r1 = GraphBackend::add_node(&mut g, make_do("root-1")).unwrap();
    let r2 = GraphBackend::add_node(&mut g, make_do("root-2")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, r1, child, EdgeKind::Ev).unwrap();

    let mut roots = GraphBackend::root_nodes(&g).unwrap();
    roots.sort_by_key(|id| id.to_string());
    let mut expected = vec![r1, r2];
    expected.sort_by_key(|id| id.to_string());

    assert_eq!(roots, expected);
    assert!(!roots.contains(&child), "child must not appear as root");
}

// ─── t071_trait_depth_returns_correct_level ───────────────────────────────────

#[test]
fn t071_trait_depth_returns_correct_level() {
    let mut g = fresh();
    let root = GraphBackend::add_node(&mut g, make_do("root")).unwrap();
    let l1 = GraphBackend::add_node(&mut g, make_do("level-1")).unwrap();
    let l2 = GraphBackend::add_node(&mut g, make_do("level-2")).unwrap();

    GraphBackend::add_edge(&mut g, root, l1, EdgeKind::Ev).unwrap();
    GraphBackend::add_edge(&mut g, l1, l2, EdgeKind::Ev).unwrap();

    assert_eq!(GraphBackend::depth(&g, root).unwrap(), 0);
    assert_eq!(GraphBackend::depth(&g, l1).unwrap(), 1);
    assert_eq!(GraphBackend::depth(&g, l2).unwrap(), 2);
}

// ─── t071_trait_remove_node_cascades_edges ────────────────────────────────────

#[test]
fn t071_trait_remove_node_cascades_edges() {
    let mut g = fresh();
    let parent = GraphBackend::add_node(&mut g, make_do("parent")).unwrap();
    let child = GraphBackend::add_node(&mut g, make_do("child")).unwrap();
    GraphBackend::add_edge(&mut g, parent, child, EdgeKind::Ev).unwrap();

    GraphBackend::remove_node(&mut g, child).unwrap();

    assert!(GraphBackend::children(&g, parent).unwrap().is_empty());
    assert_eq!(GraphBackend::node_count(&g), 1);
}

// ─── t071_trait_node_count_after_operations ───────────────────────────────────

#[test]
fn t071_trait_node_count_after_operations() {
    let mut g = fresh();
    assert_eq!(GraphBackend::node_count(&g), 0);

    let id1 = GraphBackend::add_node(&mut g, make_do("node-1")).unwrap();
    let id2 = GraphBackend::add_node(&mut g, make_do("node-2")).unwrap();
    let id3 = GraphBackend::add_node(&mut g, make_do("node-3")).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 3);

    // update_node must not change the count.
    GraphBackend::update_node(&mut g, id1, make_do("node-1-updated")).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 3);

    // remove_node decrements.
    GraphBackend::remove_node(&mut g, id2).unwrap();
    assert_eq!(GraphBackend::node_count(&g), 2);

    // all_node_ids reflects the live set.
    let mut ids = GraphBackend::all_node_ids(&g).unwrap();
    ids.sort_by_key(|id| id.to_string());
    let mut expected = vec![id1, id3];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(ids, expected);
}

// ─── t071_trait_add_and_read_contracts ───────────────────────────────────────

#[test]
fn t071_trait_add_and_read_contracts() {
    let mut g = fresh();
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

// ─── t071_trait_transaction_commits_on_success ────────────────────────────────

#[test]
fn t071_trait_transaction_commits_on_success() {
    let mut g = fresh();

    GraphBackend::begin_transaction(&mut g).unwrap();
    let id = GraphBackend::add_node(&mut g, make_do("inside transaction")).unwrap();
    GraphBackend::commit_transaction(&mut g).unwrap();

    assert!(
        GraphBackend::get_node(&g, id).unwrap().is_some(),
        "node inserted during transaction must be visible after commit"
    );
}

// ─── t071_trait_transaction_rolls_back_on_error ───────────────────────────────

/// For `AilGraph` (in-memory), `rollback_transaction` is a no-op: there is no
/// WAL or crash recovery. This test verifies the API surface is callable and
/// documents the no-op semantics explicitly.
#[test]
fn t071_trait_transaction_rolls_back_on_error() {
    let mut g = fresh();

    GraphBackend::begin_transaction(&mut g).unwrap();
    GraphBackend::add_node(&mut g, make_do("tentative node")).unwrap();
    // Simulate an error path.
    GraphBackend::rollback_transaction(&mut g).unwrap();

    // AilGraph rollback is a no-op: the node remains in memory.
    assert_eq!(
        GraphBackend::node_count(&g),
        1,
        "AilGraph rollback is a no-op; node persists (documented behaviour)"
    );
}
