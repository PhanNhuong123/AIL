use ail_graph::{ContractKind, Pattern};
use ail_text::parse;

#[test]
fn parse_wallet_fixture_full() {
    let input = r#"-- types
define WalletBalance:number where value >= 0
define PositiveAmount:number where value > 0

describe User as
  id:UserId, balance:WalletBalance, status:UserStatus

error InsufficientBalanceError
  carries current_balance:WalletBalance, requested_amount:PositiveAmount

-- main function
do transfer money safely
  from sender:User, receiver:User, amount:PositiveAmount
  -> TransferResult
  or InsufficientBalanceError

  promise before: sender.status is "active"
  promise always: sender.balance >= 0

  check sender.balance >= amount
    otherwise raise InsufficientBalanceError carries current_balance = sender.balance, requested_amount = amount

  let new_sender_balance:WalletBalance = sender.balance - amount
  let new_receiver_balance:WalletBalance = receiver.balance + amount

  update User in database where id is sender.id set balance = new_sender_balance
  update User in database where id is receiver.id set balance = new_receiver_balance

  return TransferResult
    with sender = sender, receiver = receiver, amount = amount"#;

    let g = parse(input).unwrap();

    // Count nodes: 2 define + 1 describe + 1 error + 1 do + 1 check + 2 let + 2 update + 1 return = 11
    // (promises are NOT nodes — they attach to the do node)
    assert_eq!(g.node_count(), 11);

    // Find the do node
    let do_node = g
        .all_nodes()
        .find(|n| n.pattern == Pattern::Do)
        .expect("no do node");

    // Verify contracts attached to do
    assert_eq!(do_node.contracts.len(), 2);
    assert!(do_node
        .contracts
        .iter()
        .any(|c| c.kind == ContractKind::Before));
    assert!(do_node
        .contracts
        .iter()
        .any(|c| c.kind == ContractKind::Always));

    // Verify do has params
    assert_eq!(do_node.metadata.params.len(), 3);
    assert_eq!(do_node.metadata.params[0].name, "sender");
    assert_eq!(do_node.metadata.params[0].type_ref, "User");

    // Verify do has return type
    assert!(do_node
        .metadata
        .return_type
        .as_ref()
        .unwrap()
        .contains("TransferResult"));

    // Verify do has children (check, 2x let, 2x update, return = 6)
    let children = do_node.children.as_ref().expect("do should have children");
    assert_eq!(children.len(), 6);

    // Verify child patterns in order
    let child_patterns: Vec<Pattern> = children
        .iter()
        .map(|id| g.get_node(*id).unwrap().pattern.clone())
        .collect();
    assert_eq!(
        child_patterns,
        vec![
            Pattern::Check,
            Pattern::Let,
            Pattern::Let,
            Pattern::Update,
            Pattern::Update,
            Pattern::Return,
        ]
    );

    // Check node should have otherwise metadata
    let check_node = g.get_node(children[0]).unwrap();
    assert_eq!(
        check_node.metadata.otherwise_error.as_deref(),
        Some("InsufficientBalanceError")
    );
    assert_eq!(check_node.metadata.otherwise_assigns.len(), 2);

    // Top-level: define + define + describe + error + do = 5
    // No single root (multiple top-level), so root is not set
    assert!(g.root_id().is_none());
}

#[test]
fn parse_single_file_define() {
    // Parse a single .ail fixture: just one define statement
    let g = parse("define WalletBalance:number where value >= 0").unwrap();
    assert_eq!(g.node_count(), 1);
    // Single top-level node → root is set
    assert!(g.root_id().is_some());
}

#[test]
fn parse_syntax_error_reports_span() {
    // Invalid input that pest can't parse
    let result = parse("@@@ invalid syntax @@@");
    match result {
        Err(ail_text::ParseError::SyntaxError { message, .. }) => {
            assert!(!message.is_empty());
        }
        Err(other) => panic!("expected SyntaxError, got: {other}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}
