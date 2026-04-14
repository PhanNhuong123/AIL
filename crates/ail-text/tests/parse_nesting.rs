use ail_graph::{ContractKind, Pattern};
use ail_text::{parse, ParseError};

// ── Successful nesting ───────────────────────────────────────────────────────

#[test]
fn parse_do_with_children() {
    let g = parse(
        "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult\n\n  check sender.balance >= amount\n  let new_balance:WalletBalance = sender.balance - amount",
    )
    .unwrap();

    // Do + check + let = 3 nodes
    assert_eq!(g.node_count(), 3);

    let do_node = g
        .all_nodes()
        .find(|n| n.pattern == Pattern::Do)
        .expect("no do node");
    let children = do_node.children.as_ref().expect("do should have children");
    assert_eq!(children.len(), 2);

    // Check is first child
    let first_child = g.get_node(children[0]).unwrap();
    assert_eq!(first_child.pattern, Pattern::Check);

    // Let is second child
    let second_child = g.get_node(children[1]).unwrap();
    assert_eq!(second_child.pattern, Pattern::Let);
}

#[test]
fn parse_promise_attaches_to_parent() {
    let g = parse(
        "do transfer money safely\n  from sender:User\n  -> TransferResult\n\n  promise before: sender.status is \"active\"\n  promise always: sender.balance >= 0",
    )
    .unwrap();

    // Only the Do node — promises are NOT separate nodes
    assert_eq!(g.node_count(), 1);

    let do_node = g.all_nodes().next().unwrap();
    assert_eq!(do_node.contracts.len(), 2);
    assert_eq!(do_node.contracts[0].kind, ContractKind::Before);
    assert_eq!(
        do_node.contracts[0].expression.0,
        "sender.status is \"active\""
    );
    assert_eq!(do_node.contracts[1].kind, ContractKind::Always);
}

#[test]
fn parse_nested_do_two_levels() {
    let g =
        parse("do outer task\n  from x:X\n\n  do inner task\n    from y:Y\n\n    check y.valid")
            .unwrap();

    // outer + inner + check = 3 nodes
    assert_eq!(g.node_count(), 3);

    let outer = g
        .all_nodes()
        .find(|n| n.intent == "outer task")
        .expect("no outer");
    let inner_id = outer.children.as_ref().unwrap()[0];
    let inner = g.get_node(inner_id).unwrap();
    assert_eq!(inner.intent, "inner task");

    let check_id = inner.children.as_ref().unwrap()[0];
    let check = g.get_node(check_id).unwrap();
    assert_eq!(check.pattern, Pattern::Check);
}

#[test]
fn parse_sibling_eh_edges() {
    let g = parse("do task\n  from x:X\n\n  check a.b\n  let y:Y = x.z\n  fetch z:Z from database")
        .unwrap();

    // Do + check + let + fetch = 4 nodes
    assert_eq!(g.node_count(), 4);

    let do_node = g.all_nodes().find(|n| n.pattern == Pattern::Do).unwrap();
    let children = do_node.children.as_ref().unwrap();
    assert_eq!(children.len(), 3);

    // Verify Eh edges exist between siblings
    // Total edges: 3 Ev (do→check, do→let, do→fetch) + 2 Eh (check→let, let→fetch) = 5
    assert_eq!(g.edge_count(), 5);
}

#[test]
fn parse_multiple_top_level() {
    let g = parse(
        "define WalletBalance:number where value >= 0\n\ndescribe User as\n  id:UserId, balance:WalletBalance\n\ndo transfer\n  from x:X",
    )
    .unwrap();

    assert_eq!(g.node_count(), 3);

    // Multiple top-level → no root set
    assert!(g.root_id().is_none());

    // Eh edges between top-level siblings: define→describe, describe→do = 2 edges
    assert_eq!(g.edge_count(), 2);
}

#[test]
fn parse_empty_source() {
    let g = parse("").unwrap();
    assert_eq!(g.node_count(), 0);
    assert!(g.root_id().is_none());
}

// ── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn parse_indent_error_odd() {
    let result = parse("do task\n  from x:X\n\n   check bad indent");
    match result {
        Err(ParseError::InvalidIndentation { found: 3, .. }) => {}
        Err(other) => panic!("expected InvalidIndentation(3), got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn parse_indent_error_jump() {
    let result = parse("do task\n  from x:X\n\n    check jumped indent");
    match result {
        Err(ParseError::IndentJump {
            parent_indent: 0,
            child_indent: 4,
            ..
        }) => {}
        Err(other) => panic!("expected IndentJump(0→4), got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn parse_promise_orphan_at_top_level() {
    let result = parse("promise before: x > 0");
    match result {
        Err(ParseError::PromiseWithoutDo { .. }) => {}
        Err(other) => panic!("expected PromiseWithoutDo, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn parse_promise_under_define_error() {
    let result =
        parse("define WalletBalance:number where value >= 0\n\n  promise always: value >= 0");
    match result {
        Err(ParseError::PromiseWithoutDo { .. }) => {}
        Err(other) => panic!("expected PromiseWithoutDo, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}
