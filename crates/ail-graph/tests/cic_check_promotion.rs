//! Task 8.1 — Check Promotion Algorithm tests.
//!
//! Each test is named with the `t081_` prefix as specified in the phase 8.1
//! spec. The 15 tests cover: direct sibling promotion, depth-aware ancestor
//! promotion, Rule P2 UP (check inside sibling Do), isolation rules (no
//! promotion to previous siblings, across parent scope, across branches/loops),
//! accumulation order, source-node and origin metadata, and serde roundtrip.

use ail_graph::{AilGraph, FactOrigin, NodeId, Pattern};

mod helpers;
use helpers::{make_child, make_node, make_sibling_after, set_expression};

// ─── helpers ───────────────────────────────────────────────────────────────

/// Build the first Check child under `parent` (no Eh edge — first sibling).
fn make_check_first(graph: &mut AilGraph, parent: NodeId, intent: &str, condition: &str) -> NodeId {
    let id = make_child(graph, parent, Pattern::Check, intent, None);
    set_expression(graph, id, condition);
    id
}

// ─── t081_check_promotes_to_next_sibling ──────────────────────────────────

#[test]
fn t081_check_promotes_to_next_sibling() {
    // do root
    //   check sender.balance >= amount     ← check
    //   do execute                         ← TARGET: should see the fact
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check balance", "sender.balance >= amount");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "execute", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(
        packet.promoted_facts[0].condition.0,
        "sender.balance >= amount"
    );
}

// ─── t081_check_promotes_to_all_subsequent_siblings ───────────────────────

#[test]
fn t081_check_promotes_to_all_subsequent_siblings() {
    // do root
    //   check X
    //   do a    ← sees X
    //   do b    ← sees X
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check X", "x > 0");
    let a = make_sibling_after(&mut g, check, root, Pattern::Do, "a", None);
    let b = make_sibling_after(&mut g, a, root, Pattern::Do, "b", None);
    g.set_root(root).unwrap();

    let pa = g.compute_context_packet(a).unwrap();
    let pb = g.compute_context_packet(b).unwrap();
    assert_eq!(pa.promoted_facts.len(), 1, "a should see X");
    assert_eq!(pb.promoted_facts.len(), 1, "b should see X");
    assert_eq!(pb.promoted_facts[0].condition.0, "x > 0");
}

// ─── t081_check_promotes_to_descendant_of_sibling ─────────────────────────

#[test]
fn t081_check_promotes_to_descendant_of_sibling() {
    // do root
    //   check X
    //   do outer
    //     do inner   ← TARGET: descendant of a sibling; should see X
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check X", "x > 0");
    let outer = make_sibling_after(&mut g, check, root, Pattern::Do, "outer", None);
    let inner = make_child(&mut g, outer, Pattern::Do, "inner", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(inner).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(packet.promoted_facts[0].condition.0, "x > 0");
}

// ─── t081_check_does_not_promote_to_previous_sibling ──────────────────────

#[test]
fn t081_check_does_not_promote_to_previous_sibling() {
    // do root
    //   do before    ← TARGET: comes before the check
    //   check X
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let before = make_child(&mut g, root, Pattern::Do, "before", None);
    let _check = make_sibling_after(&mut g, before, root, Pattern::Check, "check X", None);
    // Set expression on the check node
    set_expression(&mut g, _check, "x > 0");
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(before).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "node before check must not see promoted facts"
    );
}

// ─── t081_check_does_not_promote_across_parent_scope ──────────────────────

#[test]
fn t081_check_does_not_promote_across_parent_scope() {
    // do root
    //   pre (Do)      ← TARGET: precedes child_a; must not see check X
    //   child_a (Do)
    //     check X
    //
    // `pre` comes before child_a in the sibling chain. The check is inside
    // child_a's body. Because `pre` has no predecessors, its promoted_facts
    // must be empty — facts never travel backwards.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let pre = make_child(&mut g, root, Pattern::Do, "pre", None);
    let child_a = make_sibling_after(&mut g, pre, root, Pattern::Do, "child_a", None);
    let _check = make_check_first(&mut g, child_a, "check X", "x > 0");
    g.set_root(root).unwrap();

    // `pre` comes before child_a; the check is inside child_a's body.
    // `pre` must NOT see the check fact.
    let packet = g.compute_context_packet(pre).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "node preceding the Do that contains the check must not see promoted facts"
    );
}

// ─── t081_check_postcondition_promotes_up_via_rule2 ───────────────────────

#[test]
fn t081_check_postcondition_promotes_up_via_rule2() {
    // do parent
    //   do child_a
    //     check sender.status is "active"
    //   do execute        ← TARGET: should see the fact via Rule P2 UP
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "parent", None);
    let child_a = make_child(&mut g, root, Pattern::Do, "child_a", None);
    let _check = make_check_first(
        &mut g,
        child_a,
        "check status",
        r#"sender.status is "active""#,
    );
    let execute = make_sibling_after(&mut g, child_a, root, Pattern::Do, "execute", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(execute).unwrap();
    assert_eq!(
        packet.promoted_facts.len(),
        1,
        "execute should see the check inside child_a via Rule P2 UP"
    );
    assert_eq!(
        packet.promoted_facts[0].condition.0,
        r#"sender.status is "active""#
    );
}

// ─── t081_multiple_checks_accumulate_facts ────────────────────────────────

#[test]
fn t081_multiple_checks_accumulate_facts() {
    // do root
    //   check A
    //   check B
    //   do target    ← sees both A and B, in declaration order
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check_a = make_check_first(&mut g, root, "check A", "a > 0");
    let check_b = make_sibling_after(&mut g, check_a, root, Pattern::Check, "check B", None);
    set_expression(&mut g, check_b, "b > 0");
    let target = make_sibling_after(&mut g, check_b, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 2);
    // Declaration order: check_a first, check_b second.
    assert_eq!(packet.promoted_facts[0].condition.0, "a > 0");
    assert_eq!(packet.promoted_facts[1].condition.0, "b > 0");
}

// ─── t081_nested_check_scope_isolated ─────────────────────────────────────

#[test]
fn t081_nested_check_scope_isolated() {
    // do root
    //   do child_a
    //     check X   ← fact for child_a's scope (and siblings/descendants after)
    //     do after_in_a   ← sees X ✓
    //   do child_b
    //     do inner_b      ← sees X via Rule P2 UP (child_a completed first) ✓
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let child_a = make_child(&mut g, root, Pattern::Do, "child_a", None);
    let check = make_check_first(&mut g, child_a, "check X", "x > 0");
    let after_in_a = make_sibling_after(&mut g, check, child_a, Pattern::Do, "after_in_a", None);
    let child_b = make_sibling_after(&mut g, child_a, root, Pattern::Do, "child_b", None);
    let inner_b = make_child(&mut g, child_b, Pattern::Do, "inner_b", None);
    g.set_root(root).unwrap();

    // after_in_a: direct sibling after check inside child_a — sees X.
    let pa = g.compute_context_packet(after_in_a).unwrap();
    assert_eq!(pa.promoted_facts.len(), 1, "after_in_a should see X");

    // inner_b: inside child_b which comes after child_a — sees X via P2 UP.
    let pb = g.compute_context_packet(inner_b).unwrap();
    assert_eq!(pb.promoted_facts.len(), 1, "inner_b should see X via P2 UP");
}

// ─── t081_match_branch_check_isolated_to_branch ───────────────────────────

#[test]
fn t081_match_branch_check_isolated_to_branch() {
    // do root
    //   do child_a (a Match sibling — contains inner Do with a check)
    //     The check is inside a Match, so collect_facts_from_do_body stops
    //     at the Match boundary and does NOT recurse into it.
    //   do child_b   ← should NOT see the check from inside child_a's Match
    //
    // We model this as: child_a is a Do that contains a Match child (not a Do).
    // Our algorithm only recurses into Do children inside a Do body; it stops
    // at Match. So the check inside the Match is invisible to child_b.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let child_a = make_child(&mut g, root, Pattern::Do, "child_a", None);

    // Inside child_a: a Match node that wraps the check.
    let match_node = make_child(&mut g, child_a, Pattern::Match, "match status", None);
    // The check lives inside the match — collect_facts_from_do_body stops at Match.
    let _check = make_child(&mut g, match_node, Pattern::Check, "check X", None);
    set_expression(&mut g, _check, "x > 0");

    let child_b = make_sibling_after(&mut g, child_a, root, Pattern::Do, "child_b", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(child_b).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "check inside a Match must not promote outside the Match boundary"
    );
}

// ─── t081_promoted_fact_has_correct_source_node ───────────────────────────

#[test]
fn t081_promoted_fact_has_correct_source_node() {
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check X", "x > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(
        packet.promoted_facts[0].source_node, check,
        "source_node must point to the Check node itself"
    );
}

// ─── t081_promoted_fact_has_check_origin ──────────────────────────────────

#[test]
fn t081_promoted_fact_has_check_origin() {
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check X", "x > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(packet.promoted_facts[0].origin, FactOrigin::CheckPromotion);
}

// ─── t081_context_packet_includes_promoted_facts ──────────────────────────

#[test]
fn t081_context_packet_includes_promoted_facts() {
    // Verify the field exists and is populated on ContextPacket.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check balance", "sender.balance >= amount");
    let execute = make_sibling_after(&mut g, check, root, Pattern::Do, "execute", None);
    // Also confirm nodes without a preceding check have an empty field.
    let first = make_child(&mut g, root, Pattern::Let, "let x", Some("x"));
    g.set_root(root).unwrap();

    let p_execute = g.compute_context_packet(execute).unwrap();
    assert!(
        !p_execute.promoted_facts.is_empty(),
        "execute should have promoted facts"
    );

    let p_check = g.compute_context_packet(check).unwrap();
    assert!(
        p_check.promoted_facts.is_empty(),
        "check itself has no preceding checks"
    );

    let _ = first; // silence unused warning
}

// ─── t081_extract_condition_from_simple_check ─────────────────────────────

#[test]
fn t081_extract_condition_from_simple_check() {
    // A simple comparison expression is stored verbatim.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check balance", "sender.balance >= 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(packet.promoted_facts.len(), 1);
    assert_eq!(packet.promoted_facts[0].condition.0, "sender.balance >= 0");
}

// ─── t081_extract_condition_from_compound_check ───────────────────────────

#[test]
fn t081_extract_condition_from_compound_check() {
    // A compound `and` expression is stored as-is — no splitting at this layer.
    // AND-splitting for Z3 is delegated to ail-contract (task 8.3).
    let compound = "sender.balance >= amount and sender.status is \"active\"";
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check compound", compound);
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert_eq!(
        packet.promoted_facts.len(),
        1,
        "one PromotedFact, not split"
    );
    assert_eq!(packet.promoted_facts[0].condition.0, compound);
}

// ─── t081_promoted_fact_serializes_for_cache ──────────────────────────────

#[test]
fn t081_promoted_fact_serializes_for_cache() {
    // Issue 8.1-D: PromotedFact must round-trip through serde_json so it can
    // be persisted in the CIC SQLite cache.
    use ail_graph::{Expression, PromotedFact};
    let fact = PromotedFact {
        source_node: NodeId::new(),
        condition: Expression("sender.balance >= amount".to_string()),
        origin: FactOrigin::CheckPromotion,
    };

    let json = serde_json::to_string(&fact).expect("serialize PromotedFact");
    let roundtrip: PromotedFact = serde_json::from_str(&json).expect("deserialize PromotedFact");
    assert_eq!(fact, roundtrip);

    // Also verify ContextPacket serializes correctly with the new field.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check X", "x > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    let packet_json = serde_json::to_string(&packet).expect("serialize ContextPacket");
    let packet_rt: ail_graph::ContextPacket =
        serde_json::from_str(&packet_json).expect("deserialize ContextPacket");
    assert_eq!(packet.promoted_facts, packet_rt.promoted_facts);

    // Simulate old cached packet (no promoted_facts key) — must deserialize OK.
    let old_json = packet_json.replace(
        &format!(
            r#","promoted_facts":{}"#,
            serde_json::to_string(&packet.promoted_facts).unwrap()
        ),
        "",
    );
    // Guard: if the replace did not match (e.g. field order changed), the test
    // would pass vacuously. Assert the JSON actually changed.
    assert_ne!(
        old_json, packet_json,
        "string replace must have matched the promoted_facts key; check field order in ContextPacket"
    );
    let old_rt: ail_graph::ContextPacket = serde_json::from_str(&old_json)
        .expect("old packet without promoted_facts must deserialize");
    assert!(
        old_rt.promoted_facts.is_empty(),
        "old packet deserializes with empty promoted_facts via #[serde(default)]"
    );
}

// ─── t081_foreach_body_check_isolated ─────────────────────────────────────

#[test]
fn t081_foreach_body_check_isolated() {
    // do root
    //   do child_a
    //     foreach items         ← ForEach boundary: stops promotion recursion
    //       check X             ← check inside ForEach, must NOT be visible outside
    //   do child_b              ← should NOT see the check from inside the ForEach
    //
    // Mirrors t081_match_branch_check_isolated_to_branch for ForEach.
    // collect_facts_from_do_body stops at ForEach (wildcard arm); the check
    // inside is therefore invisible to child_b.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let child_a = make_child(&mut g, root, Pattern::Do, "child_a", None);
    let foreach_node = make_child(&mut g, child_a, Pattern::ForEach, "foreach items", None);
    let check = make_child(&mut g, foreach_node, Pattern::Check, "check X", None);
    set_expression(&mut g, check, "x > 0");
    let child_b = make_sibling_after(&mut g, child_a, root, Pattern::Do, "child_b", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(child_b).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "check inside a ForEach must not promote outside the ForEach boundary"
    );
}

// ─── t081_or_disjunction_in_call_shape_not_promoted ───────────────────────

#[test]
fn t081_or_disjunction_in_call_shape_not_promoted() {
    // "is_valid(sender) or amount > 0" has call-shape but is a disjunction.
    // Promoting it would be unsound: the runtime may have satisfied the check
    // via is_valid(sender) only, leaving amount > 0 unproved.
    // is_impure_function_call must treat it as impure.
    let mut g = AilGraph::new();
    let root = make_node(&mut g, Pattern::Do, "root", None);
    let check = make_check_first(&mut g, root, "check", "is_valid(sender) or amount > 0");
    let target = make_sibling_after(&mut g, check, root, Pattern::Do, "target", None);
    g.set_root(root).unwrap();

    let packet = g.compute_context_packet(target).unwrap();
    assert!(
        packet.promoted_facts.is_empty(),
        "or-disjunction with call-shape must not be promoted"
    );
}
