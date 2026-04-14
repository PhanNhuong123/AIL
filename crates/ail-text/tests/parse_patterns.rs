use ail_graph::Pattern;
use ail_text::parse;

/// Helper: parse single-statement input and return the first node's pattern + graph.
fn parse_single(input: &str) -> ail_graph::AilGraph {
    parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"))
}

fn first_node(graph: &ail_graph::AilGraph) -> &ail_graph::Node {
    graph.all_nodes().next().expect("no nodes in graph")
}

// ── Define ───────────────────────────────────────────────────────────────────

#[test]
fn parse_define_basic() {
    let g = parse_single("define WalletBalance:number where value >= 0");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Define);
    assert_eq!(n.metadata.name.as_deref(), Some("WalletBalance"));
    assert_eq!(n.metadata.base_type.as_deref(), Some("number"));
    assert!(n.expression.is_some());
    assert_eq!(n.expression.as_ref().unwrap().0, "value >= 0");
}

#[test]
fn parse_define_no_constraint() {
    let g = parse_single("define UserId:text");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Define);
    assert_eq!(n.metadata.name.as_deref(), Some("UserId"));
    assert_eq!(n.metadata.base_type.as_deref(), Some("text"));
    assert!(n.expression.is_none());
}

// ── Describe ─────────────────────────────────────────────────────────────────

#[test]
fn parse_describe_with_fields() {
    let g = parse_single("describe User as\n  id:UserId, balance:WalletBalance, status:UserStatus");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Describe);
    assert_eq!(n.metadata.name.as_deref(), Some("User"));
    assert_eq!(n.metadata.fields.len(), 3);
    assert_eq!(n.metadata.fields[0].name, "id");
    assert_eq!(n.metadata.fields[0].type_ref, "UserId");
    assert_eq!(n.metadata.fields[1].name, "balance");
    assert_eq!(n.metadata.fields[2].name, "status");
}

// ── Error ────────────────────────────────────────────────────────────────────

#[test]
fn parse_error_with_carries() {
    let g = parse_single(
        "error InsufficientBalanceError\n  carries current_balance:WalletBalance, requested_amount:PositiveAmount",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Error);
    assert_eq!(n.metadata.name.as_deref(), Some("InsufficientBalanceError"));
    assert_eq!(n.metadata.carries.len(), 2);
    assert_eq!(n.metadata.carries[0].name, "current_balance");
    assert_eq!(n.metadata.carries[0].type_ref, "WalletBalance");
}

#[test]
fn parse_error_no_carries() {
    let g = parse_single("error SimpleError");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Error);
    assert_eq!(n.metadata.name.as_deref(), Some("SimpleError"));
    assert!(n.metadata.carries.is_empty());
}

// ── Let ──────────────────────────────────────────────────────────────────────

#[test]
fn parse_let_binding() {
    let g = parse_single("let new_balance:WalletBalance = sender.balance - amount");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Let);
    assert_eq!(n.metadata.name.as_deref(), Some("new_balance"));
    assert_eq!(n.metadata.return_type.as_deref(), Some("WalletBalance"));
    assert_eq!(n.expression.as_ref().unwrap().0, "sender.balance - amount");
}

// ── Check ────────────────────────────────────────────────────────────────────

#[test]
fn parse_check_basic() {
    let g = parse_single("check sender.balance >= amount");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Check);
    assert_eq!(n.expression.as_ref().unwrap().0, "sender.balance >= amount");
    assert!(n.metadata.otherwise_error.is_none());
}

#[test]
fn parse_check_with_otherwise() {
    let g = parse_single(
        "check sender.balance >= amount\n  otherwise raise InsufficientBalanceError carries current_balance = sender.balance, requested_amount = amount",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Check);
    assert_eq!(n.expression.as_ref().unwrap().0, "sender.balance >= amount");
    assert_eq!(
        n.metadata.otherwise_error.as_deref(),
        Some("InsufficientBalanceError")
    );
    assert_eq!(n.metadata.otherwise_assigns.len(), 2);
    assert_eq!(n.metadata.otherwise_assigns[0].0, "current_balance");
    assert_eq!(n.metadata.otherwise_assigns[0].1, "sender.balance");
    assert_eq!(n.metadata.otherwise_assigns[1].0, "requested_amount");
    assert_eq!(n.metadata.otherwise_assigns[1].1, "amount");
}

#[test]
fn parse_check_otherwise_metadata() {
    let g = parse_single(
        "check sender_id is not receiver_id\n  otherwise raise InvalidTransferError carries user_id = sender_id",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Check);
    assert_eq!(
        n.metadata.otherwise_error.as_deref(),
        Some("InvalidTransferError")
    );
    assert_eq!(n.metadata.otherwise_assigns.len(), 1);
    assert_eq!(
        n.metadata.otherwise_assigns[0],
        ("user_id".to_string(), "sender_id".to_string())
    );
}

// ── Fetch ────────────────────────────────────────────────────────────────────

#[test]
fn parse_fetch_with_where() {
    let g = parse_single("fetch sender:User from database where id is sender_id");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Fetch);
    assert_eq!(n.metadata.name.as_deref(), Some("sender"));
    assert_eq!(n.metadata.return_type.as_deref(), Some("User"));
    assert!(n
        .expression
        .as_ref()
        .unwrap()
        .0
        .contains("where id is sender_id"));
}

// ── Save ─────────────────────────────────────────────────────────────────────

#[test]
fn parse_save_with_assignments() {
    let g = parse_single("save notification to queue with user_id = user.id, message = msg");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Save);
    assert_eq!(n.metadata.name.as_deref(), Some("notification"));
    let expr = n.expression.as_ref().unwrap().0.as_str();
    assert!(expr.contains("to queue"));
    assert!(expr.contains("user_id = user.id"));
}

// ── Update ───────────────────────────────────────────────────────────────────

#[test]
fn parse_update_with_where_set() {
    let g = parse_single(
        "update User in database where id is sender.id set balance = new_sender_balance",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Update);
    assert_eq!(n.metadata.name.as_deref(), Some("User"));
    let expr = n.expression.as_ref().unwrap().0.as_str();
    assert!(expr.contains("in database"));
    assert!(expr.contains("where id is sender.id"));
    assert!(expr.contains("set balance = new_sender_balance"));
}

// ── Remove ───────────────────────────────────────────────────────────────────

#[test]
fn parse_remove_basic() {
    let g = parse_single("remove Session from store where token is expired_token");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Remove);
    assert_eq!(n.metadata.name.as_deref(), Some("Session"));
    assert!(n
        .expression
        .as_ref()
        .unwrap()
        .0
        .contains("where token is expired_token"));
}

// ── Return ───────────────────────────────────────────────────────────────────

#[test]
fn parse_return_with_assignments() {
    let g = parse_single(
        "return TransferResult\n  with sender = sender, receiver = receiver, amount = amount",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Return);
    assert_eq!(n.metadata.name.as_deref(), Some("TransferResult"));
    assert_eq!(n.metadata.return_type.as_deref(), Some("TransferResult"));
    let expr = n.expression.as_ref().unwrap().0.as_str();
    assert!(expr.contains("sender = sender"));
}

// ── Raise ────────────────────────────────────────────────────────────────────

#[test]
fn parse_raise_with_carries() {
    let g = parse_single(
        "raise InsufficientBalanceError\n  carries current_balance = sender.balance, requested_amount = amount",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Raise);
    assert_eq!(n.metadata.name.as_deref(), Some("InsufficientBalanceError"));
    assert_eq!(n.metadata.carries.len(), 2);
    assert_eq!(n.metadata.carries[0].name, "current_balance");
}

// ── ForEach ──────────────────────────────────────────────────────────────────

#[test]
fn parse_for_each_typed() {
    let g = parse_single("for each item:OrderItem in order.items do calculate total");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::ForEach);
    assert_eq!(n.metadata.name.as_deref(), Some("item"));
    assert_eq!(n.metadata.params[0].type_ref, "OrderItem");
}

// ── Match ────────────────────────────────────────────────────────────────────

#[test]
fn parse_match_with_when_clauses() {
    let g = parse_single(
        "match user.status\n  when \"active\": proceed\n  when \"suspended\": raise AccountSuspendedError\n  otherwise: raise UnknownStatusError",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Match);
    let expr = n.expression.as_ref().unwrap().0.as_str();
    assert!(expr.contains("user.status"));
    assert!(expr.contains("\"active\": proceed"));
    assert!(expr.contains("otherwise: raise UnknownStatusError"));
}

// ── Together ─────────────────────────────────────────────────────────────────

#[test]
fn parse_together_inline_children() {
    let g = parse_single(
        "together\n  update User in database where id is sender.id set balance = x\n  update User in database where id is receiver.id set balance = y",
    );
    // Together node + 2 inline update children = 3 nodes
    assert_eq!(g.node_count(), 3);

    let together_node = g
        .all_nodes()
        .find(|n| n.pattern == Pattern::Together)
        .expect("no together node");
    assert_eq!(together_node.children.as_ref().map(|c| c.len()), Some(2));
}

// ── Retry ────────────────────────────────────────────────────────────────────

#[test]
fn parse_retry_with_delay() {
    let g = parse_single(
        "retry 3 times with delay 1 second\n  fetch rate:ExchangeRate from external_api where currency is \"USD\"",
    );
    assert_eq!(g.node_count(), 2); // retry + fetch child

    let retry_node = g
        .all_nodes()
        .find(|n| n.pattern == Pattern::Retry)
        .expect("no retry node");
    assert!(retry_node
        .expression
        .as_ref()
        .unwrap()
        .0
        .contains("3 times"));
    assert_eq!(retry_node.children.as_ref().map(|c| c.len()), Some(1));
}

// ── Do ───────────────────────────────────────────────────────────────────────

#[test]
fn parse_do_with_params_and_return() {
    let g = parse_single(
        "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult or InsufficientBalanceError",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Do);
    assert_eq!(n.intent, "transfer money safely");
    assert_eq!(n.metadata.params.len(), 3);
    assert_eq!(n.metadata.params[0].name, "sender");
    assert_eq!(n.metadata.params[0].type_ref, "User");
    assert!(n
        .metadata
        .return_type
        .as_ref()
        .unwrap()
        .contains("TransferResult"));
}

#[test]
fn parse_do_document_style() {
    let g = parse_single(
        "transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult",
    );
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Do);
    assert_eq!(n.intent, "transfer money safely");
    assert_eq!(n.metadata.params.len(), 3);
}

#[test]
fn parse_ambiguous_intent_becomes_do() {
    // "walk" is NOT a reserved keyword prefix, so it becomes a document-style do
    let g = parse_single("walk around carefully");
    let n = first_node(&g);
    assert_eq!(n.pattern, Pattern::Do);
    assert_eq!(n.intent, "walk around carefully");
}
