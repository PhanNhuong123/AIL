//! Task 8.2 — Path-Aware Context Packets tests.
//!
//! Tests are named with the `t082_` prefix. The 9 tests cover:
//! - Context packet promoted_facts field presence and values.
//! - Ordering, source node ID, and serde roundtrip.
//! - Cache invalidation helper: change, removal, and insertion scenarios.

use ail_graph::{
    check_promotion_affected_nodes, AilGraph, EdgeKind, Expression, FactOrigin, NodeId, Pattern,
    PromotedFact,
};

mod helpers;
use helpers::{make_child, make_node, make_sibling_after, set_expression};

// ─── local builder helpers ────────────────────────────────────────────────────

/// First Check child under `parent` (no Eh predecessor).
fn make_check(graph: &mut AilGraph, parent: NodeId, condition: &str) -> NodeId {
    let id = make_child(graph, parent, Pattern::Check, "check condition", None);
    set_expression(graph, id, condition);
    id
}

// ─── t082_context_packet_includes_promoted_facts_field ───────────────────────

#[test]
fn t082_context_packet_includes_promoted_facts_field() {
    // do root
    //   check x > 0
    //   do target      ← promoted_facts must be non-empty
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check(&mut g, root, "x > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert!(
        !packet.promoted_facts.is_empty(),
        "promoted_facts must be non-empty after a preceding check"
    );
    assert_eq!(packet.promoted_facts[0].condition.0, "x > 0");
}

// ─── t082_empty_promoted_facts_when_no_checks ────────────────────────────────

#[test]
fn t082_empty_promoted_facts_when_no_checks() {
    // do root
    //   do a     ← no preceding check; promoted_facts must be empty
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let a = make_child(&mut g, root, Pattern::Do, "a", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(a).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "no preceding checks → empty promoted_facts"
    );
}

// ─── t082_promoted_facts_ordered_by_check_position ───────────────────────────

#[test]
fn t082_promoted_facts_ordered_by_check_position() {
    // do root
    //   check a > 0   ← first
    //   check b > 0   ← second
    //   do target     ← should see [a > 0, b > 0] in order
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let c1 = make_check(&mut g, root, "a > 0");
    let c2 = make_sibling_after(&mut g, c1, root, Pattern::Check, "check b", None);
    set_expression(&mut g, c2, "b > 0");
    let target = make_sibling_after(&mut g, c2, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 2);
    assert_eq!(packet.promoted_facts[0].condition.0, "a > 0");
    assert_eq!(packet.promoted_facts[1].condition.0, "b > 0");
}

// ─── t082_promoted_facts_include_source_node_id ──────────────────────────────

#[test]
fn t082_promoted_facts_include_source_node_id() {
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check(&mut g, root, "amount > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(
        packet.promoted_facts[0].source_node, check,
        "source_node must equal the check NodeId"
    );
    assert_eq!(packet.promoted_facts[0].origin, FactOrigin::CheckPromotion);
}

// ─── t082_context_packet_roundtrip_with_promoted_facts ───────────────────────

#[test]
fn t082_context_packet_roundtrip_with_promoted_facts() {
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check(&mut g, root, "sender.balance >= amount");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "execute", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert!(!packet.promoted_facts.is_empty());

    let json = serde_json::to_string(&packet).expect("serialize ContextPacket");
    let rt: ail_graph::ContextPacket =
        serde_json::from_str(&json).expect("deserialize ContextPacket");

    assert_eq!(packet.promoted_facts, rt.promoted_facts);
    assert_eq!(rt.promoted_facts[0].condition.0, "sender.balance >= amount");
}

// ─── t082_promoted_fact_serde_roundtrip ──────────────────────────────────────

#[test]
fn t082_promoted_fact_serde_roundtrip() {
    // Bare PromotedFact serde roundtrip — validates the JSON shape that a
    // future SQLite cache would write and read back.
    let fact = PromotedFact {
        source_node: NodeId::new(),
        condition: Expression("balance >= 0".to_string()),
        origin: FactOrigin::CheckPromotion,
    };

    let json = serde_json::to_string(&fact).expect("serialize PromotedFact");
    let rt: PromotedFact = serde_json::from_str(&json).expect("deserialize PromotedFact");
    assert_eq!(fact, rt);
}

// ─── t082_cache_invalidated_on_check_change ──────────────────────────────────

#[test]
fn t082_cache_invalidated_on_check_change() {
    // do root
    //   check X
    //   do sib_a     ← affected (direct sibling after check)
    //     do inner   ← affected (descendant of sib_a)
    //   do sib_b     ← affected (second sibling after check)
    //
    // When the check expression changes, sib_a, inner, and sib_b all need
    // packet recomputation.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check(&mut g, root, "x > 0");
    let sib_a = make_sibling_after(&mut g, check, root, Pattern::Do, "sib_a", None);
    let inner = make_child(&mut g, sib_a, Pattern::Do, "inner", None);
    let sib_b = make_sibling_after(&mut g, sib_a, root, Pattern::Do, "sib_b", None);
    g.set_root(root).unwrap();

    let affected = check_promotion_affected_nodes(&g, check).unwrap();

    assert!(affected.contains(&sib_a), "sib_a must be in affected set");
    assert!(affected.contains(&inner), "inner must be in affected set");
    assert!(affected.contains(&sib_b), "sib_b must be in affected set");
    // The check itself and root are not in the affected set.
    assert!(!affected.contains(&check));
    assert!(!affected.contains(&root));
}

// ─── t082_cache_invalidated_on_check_removal ─────────────────────────────────

#[test]
fn t082_cache_invalidated_on_check_removal() {
    // The helper must be called BEFORE removing the check node (while it is
    // still in the graph). The returned set is the scope that a cache layer
    // must invalidate before the removal mutation is applied.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check(&mut g, root, "balance >= 0");
    let sib = make_sibling_after(&mut g, check, root, Pattern::Do, "execute", None);
    g.set_root(root).unwrap();

    // Snapshot affected set before removal.
    let affected = check_promotion_affected_nodes(&g, check).unwrap();
    assert!(
        !affected.is_empty(),
        "affected set must be non-empty before removal"
    );
    assert!(affected.contains(&sib));

    // Perform the mutation: remove the check node.
    // (In a real cache layer, the affected nodes are invalidated before this.)
    g.remove_node(check).unwrap();

    // Confirm the sibling still exists in the graph — it was valid at snapshot time.
    assert!(
        g.get_node(sib).is_ok(),
        "sib must still exist after check removal"
    );
}

// ─── t082_cache_invalidated_on_check_insertion ───────────────────────────────

#[test]
fn t082_cache_invalidated_on_check_insertion() {
    // Start with: root → sib_a → sib_b (Eh chain).
    // Insert a new check between root and sib_a (i.e., the check becomes the
    // first child, sib_a follows it via Eh).
    //
    // After insertion, check_promotion_affected_nodes(check) should return
    // {sib_a, sib_b} — the nodes that now receive a new promoted fact.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    // Build sib_a as the first child of root (no Eh predecessor).
    let sib_a = make_child(&mut g, root, Pattern::Do, "sib_a", None);
    let sib_b = make_sibling_after(&mut g, sib_a, root, Pattern::Do, "sib_b", None);
    g.set_root(root).unwrap();

    // Insert a new check as first child; wire it before sib_a via Eh.
    let check = make_child(&mut g, root, Pattern::Check, "new check", None);
    set_expression(&mut g, check, "amount > 0");
    // Wire: check → sib_a via Eh (check is now the Eh predecessor of sib_a).
    g.add_edge(check, sib_a, EdgeKind::Eh).unwrap();

    let affected = check_promotion_affected_nodes(&g, check).unwrap();

    assert!(
        affected.contains(&sib_a),
        "sib_a must be invalidated after check insertion"
    );
    assert!(
        affected.contains(&sib_b),
        "sib_b must be invalidated after check insertion"
    );
    assert!(!affected.contains(&check));
    assert!(!affected.contains(&root));
}

// ─── t082_cache_invalidated_for_nested_check_in_do ──────────────────────────

#[test]
fn t082_cache_invalidated_for_nested_check_in_do() {
    // do root
    //   do validate [V]
    //     check x > 0 [C]  ← check changes
    //   do execute [E]      ← must be invalidated (receives promoted fact from C)
    //     fetch data [F]    ← must be invalidated (descendant of E)
    //
    // The ancestor walk from C:
    //   Level C: siblings_after(C) inside V = [] (no siblings after C in V)
    //   Level V: siblings_after(V) under root = [E] → E + descendants(E) = [E, F]
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let validate = make_child(&mut g, root, Pattern::Do, "validate", None);
    let check = make_check(&mut g, validate, "x > 0");
    let execute = make_sibling_after(&mut g, validate, root, Pattern::Do, "execute", None);
    let fetch = make_child(&mut g, execute, Pattern::Fetch, "fetch data", Some("data"));
    g.set_root(root).unwrap();

    let affected = check_promotion_affected_nodes(&g, check).unwrap();

    assert!(
        affected.contains(&execute),
        "execute must be in affected set (sibling of check's parent Do)"
    );
    assert!(
        affected.contains(&fetch),
        "fetch must be in affected set (descendant of execute)"
    );
    assert!(!affected.contains(&check));
    assert!(!affected.contains(&validate));
}

// ─── t082_cache_invalidated_for_deeply_nested_check ─────────────────────────

#[test]
fn t082_cache_invalidated_for_deeply_nested_check() {
    // do root
    //   do level_1
    //     do level_2
    //       check x > 0 [C]   ← check changes
    //     do sib_of_l2 [S2]   ← must be invalidated
    //   do sib_of_l1 [S1]     ← must be invalidated
    //
    // The ancestor walk from C:
    //   Level C: siblings_after(C) in level_2 = [] (no siblings)
    //   Level level_2: siblings_after(level_2) in level_1 = [S2]
    //   Level level_1: siblings_after(level_1) in root = [S1]
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let l1 = make_child(&mut g, root, Pattern::Do, "level_1", None);
    let l2 = make_child(&mut g, l1, Pattern::Do, "level_2", None);
    let check = make_check(&mut g, l2, "x > 0");
    let s2 = make_sibling_after(&mut g, l2, l1, Pattern::Do, "sib_of_l2", None);
    let s1 = make_sibling_after(&mut g, l1, root, Pattern::Do, "sib_of_l1", None);
    g.set_root(root).unwrap();

    let affected = check_promotion_affected_nodes(&g, check).unwrap();

    assert!(affected.contains(&s2), "sib_of_l2 must be invalidated");
    assert!(affected.contains(&s1), "sib_of_l1 must be invalidated");
    assert!(!affected.contains(&check));
    assert!(!affected.contains(&l2));
    assert!(!affected.contains(&l1));
}
