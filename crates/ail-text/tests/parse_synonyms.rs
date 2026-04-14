use ail_graph::{ContractKind, Pattern};
use ail_text::parse;

#[test]
fn parse_requires_as_before() {
    let g =
        parse("do validate\n  from x:X\n\n  requires that sender.status is \"active\"").unwrap();

    let do_node = g.all_nodes().next().unwrap();
    assert_eq!(do_node.contracts.len(), 1);
    assert_eq!(do_node.contracts[0].kind, ContractKind::Before);
    assert!(do_node.contracts[0].expression.0.contains("sender.status"));
}

#[test]
fn parse_guarantees_as_after() {
    let g = parse("do validate\n  from x:X\n\n  guarantees that balance decreased").unwrap();

    let do_node = g.all_nodes().next().unwrap();
    assert_eq!(do_node.contracts.len(), 1);
    assert_eq!(do_node.contracts[0].kind, ContractKind::After);
}

#[test]
fn parse_must_always_as_always() {
    let g = parse("do validate\n  from x:X\n\n  must always satisfy balance >= 0").unwrap();

    let do_node = g.all_nodes().next().unwrap();
    assert_eq!(do_node.contracts.len(), 1);
    assert_eq!(do_node.contracts[0].kind, ContractKind::Always);
}

#[test]
fn parse_ensure_as_check() {
    let g = parse("ensure sender.balance >= amount").unwrap();
    let n = g.all_nodes().next().unwrap();
    assert_eq!(n.pattern, Pattern::Check);
}

#[test]
fn parse_look_up_as_fetch() {
    let g = parse("look up sender:User from database where id is sender_id").unwrap();
    let n = g.all_nodes().next().unwrap();
    assert_eq!(n.pattern, Pattern::Fetch);
    assert_eq!(n.metadata.name.as_deref(), Some("sender"));
}

#[test]
fn parse_atomically_as_together() {
    let g = parse(
        "atomically\n  update User in database where id is sender.id set balance = x\n  update User in database where id is receiver.id set balance = y",
    )
    .unwrap();

    let together = g
        .all_nodes()
        .find(|n| n.pattern == Pattern::Together)
        .unwrap();
    assert_eq!(together.children.as_ref().map(|c| c.len()), Some(2));
}
