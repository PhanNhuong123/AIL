use ail_graph::{AilGraph, Contract, ContractKind, Expression, Node, NodeId, Pattern};
use ail_text::{parse, render};

/// Helper: parse → render at full depth.
fn parse_render(input: &str) -> String {
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));
    render(&graph, usize::MAX)
}

/// Helper: parse → render at given depth.
fn parse_render_depth(input: &str, depth: usize) -> String {
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));
    render(&graph, depth)
}

// ══════════════════════════════════════════════════════════════════════════════
// Pattern tests — parse source → render → verify canonical output
// ══════════════════════════════════════════════════════════════════════════════

// ── Define ───────────────────────────────────────────────────────────────────

#[test]
fn render_define_with_constraint() {
    let out = parse_render("define WalletBalance:number where value >= 0");
    assert_eq!(out.trim(), "define WalletBalance:number where value >= 0");
}

#[test]
fn render_define_no_constraint() {
    let out = parse_render("define UserId:text");
    assert_eq!(out.trim(), "define UserId:text");
}

// ── Describe ─────────────────────────────────────────────────────────────────

#[test]
fn render_describe_with_fields() {
    let out =
        parse_render("describe User as\n  id:UserId, balance:WalletBalance, status:UserStatus");
    assert_eq!(
        out.trim(),
        "describe User as\n  id:UserId, balance:WalletBalance, status:UserStatus"
    );
}

// ── Error ────────────────────────────────────────────────────────────────────

#[test]
fn render_error_with_carries() {
    let out = parse_render(
        "error InsufficientBalanceError\n  carries current_balance:WalletBalance, requested_amount:PositiveAmount",
    );
    assert_eq!(
        out.trim(),
        "error InsufficientBalanceError\n  carries current_balance:WalletBalance, requested_amount:PositiveAmount"
    );
}

#[test]
fn render_error_no_carries() {
    let out = parse_render("error SimpleError");
    assert_eq!(out.trim(), "error SimpleError");
}

// ── Do ───────────────────────────────────────────────────────────────────────

#[test]
fn render_do_with_params_and_return() {
    let out = parse_render(
        "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult",
    );
    assert_eq!(
        out.trim(),
        "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult"
    );
}

#[test]
fn render_do_with_or_error() {
    let out = parse_render(
        "do transfer money\n  from sender:User, amount:PositiveAmount\n  -> TransferResult or InsufficientBalanceError",
    );
    let expected = "do transfer money\n  from sender:User, amount:PositiveAmount\n  -> TransferResult\n  or InsufficientBalanceError";
    assert_eq!(out.trim(), expected);
}

#[test]
fn render_do_multiline_or_errors() {
    let out = parse_render(
        "do process payment\n  from sender:User, amount:PositiveAmount\n  -> PaymentResult or InsufficientBalanceError\n  or InvalidUserError",
    );
    let expected = "do process payment\n  from sender:User, amount:PositiveAmount\n  -> PaymentResult\n  or InsufficientBalanceError\n  or InvalidUserError";
    assert_eq!(out.trim(), expected);
}

// ── Let ──────────────────────────────────────────────────────────────────────

#[test]
fn render_let_binding() {
    let out = parse_render("let new_balance:WalletBalance = sender.balance - amount");
    assert_eq!(
        out.trim(),
        "let new_balance:WalletBalance = sender.balance - amount"
    );
}

// ── Check ────────────────────────────────────────────────────────────────────

#[test]
fn render_check_basic() {
    let out = parse_render("check sender.balance >= amount");
    assert_eq!(out.trim(), "check sender.balance >= amount");
}

#[test]
fn render_check_with_otherwise() {
    let out = parse_render(
        "check sender.balance >= amount\n  otherwise raise InsufficientBalanceError carries current_balance = sender.balance, requested_amount = amount",
    );
    assert_eq!(
        out.trim(),
        "check sender.balance >= amount\n  otherwise raise InsufficientBalanceError carries current_balance = sender.balance, requested_amount = amount"
    );
}

// ── ForEach ──────────────────────────────────────────────────────────────────

#[test]
fn render_for_each_typed() {
    let out = parse_render("for each item:OrderItem in order.items do calculate total");
    assert_eq!(
        out.trim(),
        "for each item:OrderItem in order.items do calculate total"
    );
}

#[test]
fn render_for_each_with_do_intent() {
    let out = parse_render("for each entry:LogEntry in audit.log do validate entry");
    assert_eq!(
        out.trim(),
        "for each entry:LogEntry in audit.log do validate entry"
    );
}

// ── Match ────────────────────────────────────────────────────────────────────

#[test]
fn render_match_with_when_clauses() {
    let out = parse_render(
        "match user.status\n  when \"active\": proceed\n  when \"suspended\": raise AccountSuspendedError\n  otherwise: raise UnknownStatusError",
    );
    let expected = "match user.status\n  when \"active\": proceed\n  when \"suspended\": raise AccountSuspendedError\n  otherwise: raise UnknownStatusError";
    assert_eq!(out.trim(), expected);
}

// ── Fetch ────────────────────────────────────────────────────────────────────

#[test]
fn render_fetch_with_where() {
    let out = parse_render("fetch sender:User from database where id is sender_id");
    assert_eq!(
        out.trim(),
        "fetch sender:User from database where id is sender_id"
    );
}

// ── Save ─────────────────────────────────────────────────────────────────────

#[test]
fn render_save_with_assignments() {
    let out = parse_render("save notification to queue with user_id = user.id, message = msg");
    assert_eq!(
        out.trim(),
        "save notification to queue with user_id = user.id, message = msg"
    );
}

// ── Update ───────────────────────────────────────────────────────────────────

#[test]
fn render_update_with_where_set() {
    let out = parse_render(
        "update User in database where id is sender.id set balance = new_sender_balance",
    );
    assert_eq!(
        out.trim(),
        "update User in database where id is sender.id set balance = new_sender_balance"
    );
}

// ── Remove ───────────────────────────────────────────────────────────────────

#[test]
fn render_remove_basic() {
    let out = parse_render("remove Session from store where token is expired_token");
    assert_eq!(
        out.trim(),
        "remove Session from store where token is expired_token"
    );
}

// ── Return ───────────────────────────────────────────────────────────────────

#[test]
fn render_return_with_assignments() {
    let out = parse_render(
        "return TransferResult\n  with sender = sender, receiver = receiver, amount = amount",
    );
    assert_eq!(
        out.trim(),
        "return TransferResult\n  with sender = sender, receiver = receiver, amount = amount"
    );
}

#[test]
fn render_return_simple() {
    let out = parse_render("return TransferResult");
    assert_eq!(out.trim(), "return TransferResult");
}

// ── Raise ────────────────────────────────────────────────────────────────────

#[test]
fn render_raise_with_carries() {
    let out = parse_render(
        "raise InsufficientBalanceError\n  carries current_balance = sender.balance, requested_amount = amount",
    );
    assert_eq!(
        out.trim(),
        "raise InsufficientBalanceError\n  carries current_balance = sender.balance, requested_amount = amount"
    );
}

// ── Together ─────────────────────────────────────────────────────────────────

#[test]
fn render_together_with_children() {
    let out = parse_render(
        "together\n  update User in database where id is sender.id set balance = x\n  update User in database where id is receiver.id set balance = y",
    );
    let expected = "\
together
  update User in database where id is sender.id set balance = x
  update User in database where id is receiver.id set balance = y";
    assert_eq!(out.trim(), expected);
}

// ── Retry ────────────────────────────────────────────────────────────────────

#[test]
fn render_retry_with_delay() {
    let out = parse_render(
        "retry 3 times with delay 1 second\n  fetch rate:ExchangeRate from external_api where currency is \"USD\"",
    );
    let expected = "\
retry 3 times with delay 1 second
  fetch rate:ExchangeRate from external_api where currency is \"USD\"";
    assert_eq!(out.trim(), expected);
}

// ══════════════════════════════════════════════════════════════════════════════
// Depth control tests
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn render_depth_zero_collapses_children() {
    let input = "\
do transfer money
  from sender:User, amount:PositiveAmount
  -> TransferResult

  promise before: sender.balance >= amount

  let new_balance:WalletBalance = sender.balance - amount";

    let out = parse_render_depth(input, 0);
    // depth=0: only the do signature, no contracts or children
    assert!(out.contains("do transfer money"));
    assert!(!out.contains("promise"));
    assert!(!out.contains("let new_balance"));
}

#[test]
fn render_depth_zero_omits_contracts() {
    let input = "\
do validate user
  from user_id:UserId
  -> User

  promise before: user_id is not empty";

    let out = parse_render_depth(input, 0);
    assert!(out.contains("do validate user"));
    assert!(!out.contains("promise"));
}

#[test]
fn render_depth_one_shows_contracts() {
    let input = "\
do transfer money
  from sender:User, amount:PositiveAmount
  -> TransferResult

  promise before: sender.balance >= amount
  promise after: sender.balance >= 0

  let new_balance:WalletBalance = sender.balance - amount";

    let out = parse_render_depth(input, 1);
    assert!(out.contains("do transfer money"));
    assert!(out.contains("promise before: sender.balance >= amount"));
    assert!(out.contains("promise after: sender.balance >= 0"));
    assert!(out.contains("let new_balance"));
}

#[test]
fn render_depth_one_shows_children() {
    let input = "\
do transfer money
  from sender:User, amount:PositiveAmount
  -> TransferResult

  let new_balance:WalletBalance = sender.balance - amount";

    let out = parse_render_depth(input, 1);
    assert!(out.contains("let new_balance"));
}

#[test]
fn render_depth_max_expands_all() {
    let input = "\
do transfer money
  from sender:User, amount:PositiveAmount
  -> TransferResult

  promise before: sender.balance >= amount

  let new_balance:WalletBalance = sender.balance - amount";

    let out = parse_render_depth(input, usize::MAX);
    assert!(out.contains("do transfer money"));
    assert!(out.contains("promise before"));
    assert!(out.contains("let new_balance"));
}

// ══════════════════════════════════════════════════════════════════════════════
// Structure and ordering tests
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn render_multiple_top_level_nodes() {
    let input = "define WalletBalance:number where value >= 0\ndefine UserId:text";
    let out = parse_render(input);
    // Two top-level nodes separated by a blank line
    assert!(out.contains("define WalletBalance:number where value >= 0"));
    assert!(out.contains("define UserId:text"));
    // Blank line between them
    assert!(out.contains("\n\n"));
}

#[test]
fn render_trailing_newline() {
    let out = parse_render("define UserId:text");
    assert!(out.ends_with('\n'));
}

#[test]
fn render_empty_graph() {
    let graph = AilGraph::new();
    let out = render(&graph, usize::MAX);
    assert!(out.is_empty());
}

#[test]
fn render_indentation_two_spaces() {
    let input = "\
do transfer money
  from sender:User, amount:PositiveAmount
  -> TransferResult

  let new_balance:WalletBalance = sender.balance - amount";

    let out = parse_render_depth(input, usize::MAX);
    // Children should be indented with exactly 2 spaces
    let lines: Vec<&str> = out.lines().collect();
    let let_line = lines
        .iter()
        .find(|l| l.contains("let new_balance"))
        .expect("let line missing");
    assert!(
        let_line.starts_with("  let"),
        "expected 2-space indent, got: {let_line:?}"
    );
}

#[test]
fn render_contracts_sorted_by_kind() {
    // Build a graph with contracts in non-canonical order: After, Always, Before
    let mut graph = AilGraph::new();
    let id = NodeId::new();
    let mut node = Node::new(id, "test action", Pattern::Do);
    node.contracts = vec![
        Contract {
            kind: ContractKind::After,
            expression: Expression("result > 0".to_string()),
        },
        Contract {
            kind: ContractKind::Always,
            expression: Expression("balance >= 0".to_string()),
        },
        Contract {
            kind: ContractKind::Before,
            expression: Expression("input > 0".to_string()),
        },
    ];
    graph.add_node(node).unwrap();
    graph.set_root(id).unwrap();

    let out = render(&graph, usize::MAX);
    let lines: Vec<&str> = out.lines().collect();
    let contract_lines: Vec<&&str> = lines.iter().filter(|l| l.contains("promise")).collect();

    assert_eq!(contract_lines.len(), 3);
    assert!(contract_lines[0].contains("promise before:"));
    assert!(contract_lines[1].contains("promise after:"));
    assert!(contract_lines[2].contains("promise always:"));
}

#[test]
fn render_directory_container_skipped() {
    // Build a graph with a container Describe (name=None) as root
    let mut graph = AilGraph::new();

    let container_id = NodeId::new();
    let mut container = Node::new(container_id, "container", Pattern::Describe);
    container.metadata.name = None;
    graph.add_node(container).unwrap();

    let child_id = NodeId::new();
    let mut child = Node::new(child_id, "define WalletBalance", Pattern::Define);
    child.metadata.name = Some("WalletBalance".to_string());
    child.metadata.base_type = Some("number".to_string());
    child.expression = Some(Expression("value >= 0".to_string()));
    graph.add_node(child).unwrap();

    graph
        .add_edge(container_id, child_id, ail_graph::EdgeKind::Ev)
        .unwrap();
    let container_node = graph.get_node_mut(container_id).unwrap();
    container_node.children = Some(vec![child_id]);

    graph.set_root(container_id).unwrap();

    let out = render(&graph, usize::MAX);
    // Container node text should NOT appear
    assert!(!out.contains("describe"));
    // Child should render directly
    assert!(out.contains("define WalletBalance:number where value >= 0"));
}

// ══════════════════════════════════════════════════════════════════════════════
// Roundtrip tests — parse → render → verify structure
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn render_roundtrip_define() {
    let input = "define WalletBalance:number where value >= 0\n";
    let out = parse_render(input);
    assert_eq!(out, input);
}

#[test]
fn render_roundtrip_do_with_body() {
    // Parse a do with contracts and children, then render.
    let input = "\
do transfer money
  from sender_balance:WalletBalance, amount:PositiveAmount
  -> WalletBalance

  promise before: sender_balance >= amount
  promise before: amount > 0
  promise after: sender_balance >= 0

  let new_balance:WalletBalance = sender_balance - amount
";

    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));
    let out = render(&graph, usize::MAX);
    assert_eq!(out, input);
}

#[test]
fn render_roundtrip_wallet_full() {
    use ail_text::parse_directory;
    use std::path::Path;

    let fixture_dir = Path::new("tests/fixtures/wallet_full");
    if !fixture_dir.exists() {
        // Skip if fixture not available in test environment
        return;
    }

    let graph =
        parse_directory(fixture_dir).unwrap_or_else(|e| panic!("parse_directory failed: {e}"));
    let out = render(&graph, usize::MAX);

    // The rendered output should contain all patterns from the fixtures
    assert!(out.contains("define WalletBalance:number where value >= 0"));
    assert!(out.contains("define PositiveAmount:number where value > 0"));
    assert!(out.contains("describe User as"));
    assert!(out.contains("describe TransferResult as"));
    assert!(out.contains("do transfer money"));
    assert!(out.contains("promise before:"));
    assert!(out.contains("let new_balance:WalletBalance"));

    // Re-parse the rendered output should succeed (structural validity)
    let reparsed = parse(&out);
    assert!(
        reparsed.is_ok(),
        "re-parsing rendered wallet_full failed: {:?}",
        reparsed.err()
    );
}
