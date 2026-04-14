use ail_text::{AilParser, Rule};
use pest::Parser;

// ── Helper ────────────────────────────────────────────────────────────────────

fn parse_file(input: &str) {
    AilParser::parse(Rule::file, input)
        .unwrap_or_else(|e| panic!("parse failed:\n{e}"));
}

fn parse_stmt(input: &str) -> Rule {
    let pairs = AilParser::parse(Rule::file, input)
        .unwrap_or_else(|e| panic!("parse failed:\n{e}"));
    // file → statement → line_indent + statement_body
    // walk down to the statement_body's inner rule
    let stmt = pairs
        .into_iter()
        .next()
        .expect("expected at least one pair from file");
    // stmt is Rule::file — descend
    let statement = stmt
        .into_inner()
        .find(|p| p.as_rule() == Rule::statement)
        .expect("expected statement inside file");
    let body = statement
        .into_inner()
        .find(|p| p.as_rule() == Rule::statement_body)
        .expect("expected statement_body inside statement");
    body.into_inner()
        .next()
        .expect("expected pattern rule inside statement_body")
        .as_rule()
}

// ── Fixture-driven tests (all code-style) ─────────────────────────────────────

#[test]
fn parse_define_wallet_balance() {
    assert_eq!(
        parse_stmt("define WalletBalance:number where value >= 0"),
        Rule::define_stmt
    );
}

#[test]
fn parse_define_with_regex() {
    assert_eq!(
        parse_stmt(r#"define UserId:text where value matches /usr_[a-z0-9]+/"#),
        Rule::define_stmt
    );
}

#[test]
fn parse_define_with_set() {
    assert_eq!(
        parse_stmt(r#"define UserStatus:text where value in {"active", "locked"}"#),
        Rule::define_stmt
    );
}

#[test]
fn parse_define_positive_amount() {
    assert_eq!(
        parse_stmt("define PositiveAmount:number where value > 0"),
        Rule::define_stmt
    );
}

#[test]
fn parse_describe_user() {
    assert_eq!(
        parse_stmt("describe User as\n  id:UserId, balance:WalletBalance, status:UserStatus"),
        Rule::describe_stmt
    );
}

#[test]
fn parse_describe_transfer_result() {
    assert_eq!(
        parse_stmt("describe TransferResult as\n  sender:User, receiver:User, amount:PositiveAmount"),
        Rule::describe_stmt
    );
}

#[test]
fn parse_error_insufficient_balance() {
    assert_eq!(
        parse_stmt(
            "error InsufficientBalanceError\n  carries current_balance:WalletBalance, requested_amount:PositiveAmount"
        ),
        Rule::error_stmt
    );
}

#[test]
fn parse_check_sender_id_not_receiver() {
    assert_eq!(
        parse_stmt(
            "check sender_id is not receiver_id\n  otherwise raise InvalidTransferError carries user_id = sender_id"
        ),
        Rule::check_stmt
    );
}

#[test]
fn parse_fetch_sender() {
    assert_eq!(
        parse_stmt("fetch sender:User from database where id is sender_id"),
        Rule::fetch_stmt
    );
}

#[test]
fn parse_let_new_sender_balance() {
    assert_eq!(
        parse_stmt("let new_sender_balance:WalletBalance = sender.balance - amount"),
        Rule::let_stmt
    );
}

#[test]
fn parse_update_sender() {
    assert_eq!(
        parse_stmt(
            "update User in database where id is sender.id set balance = new_sender_balance"
        ),
        Rule::update_stmt
    );
}

#[test]
fn parse_return_transfer_result() {
    assert_eq!(
        parse_stmt(
            "return TransferResult\n  with sender = sender, receiver = receiver, amount = amount"
        ),
        Rule::return_stmt
    );
}

// ── Non-fixture pattern tests ─────────────────────────────────────────────────

#[test]
fn parse_do_code_style() {
    assert_eq!(
        parse_stmt(
            "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult"
        ),
        Rule::do_stmt
    );
}

#[test]
fn parse_promise_before() {
    assert_eq!(
        parse_stmt(r#"promise before: sender.status is "active""#),
        Rule::promise_stmt
    );
}

#[test]
fn parse_promise_after() {
    assert_eq!(
        parse_stmt("promise after: result is old(balance) - amount"),
        Rule::promise_stmt
    );
}

#[test]
fn parse_promise_always() {
    assert_eq!(
        parse_stmt("promise always: balance >= 0"),
        Rule::promise_stmt
    );
}

#[test]
fn parse_save_stmt() {
    assert_eq!(
        parse_stmt(
            "save notification to queue with user_id = user.id, message = msg"
        ),
        Rule::save_stmt
    );
}

#[test]
fn parse_remove_stmt() {
    assert_eq!(
        parse_stmt("remove Session from store where token is expired_token"),
        Rule::remove_stmt
    );
}

#[test]
fn parse_raise_stmt() {
    assert_eq!(
        parse_stmt(
            "raise InsufficientBalanceError\n  carries current_balance = sender.balance, requested_amount = amount"
        ),
        Rule::raise_stmt
    );
}

#[test]
fn parse_together_stmt() {
    assert_eq!(
        parse_stmt(
            "together\n  update User in database where id is sender.id set balance = new_sender_balance\n  update User in database where id is receiver.id set balance = new_receiver_balance"
        ),
        Rule::together_stmt
    );
}

#[test]
fn parse_retry_stmt() {
    assert_eq!(
        parse_stmt(
            "retry 3 times with delay 1 second\n  fetch rate:ExchangeRate from external_api where currency is \"USD\""
        ),
        Rule::retry_stmt
    );
}

#[test]
fn parse_for_each_stmt() {
    assert_eq!(
        parse_stmt("for each item:OrderItem in order.items do calculate total"),
        Rule::for_each_stmt
    );
}

#[test]
fn parse_match_stmt() {
    assert_eq!(
        parse_stmt(
            "match user.status\n  when \"active\": proceed\n  when \"suspended\": raise AccountSuspendedError\n  otherwise: raise UnknownStatusError"
        ),
        Rule::match_stmt
    );
}

// ── Synonym tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_synonym_requires() {
    assert_eq!(
        parse_stmt("requires that sender is active"),
        Rule::promise_stmt
    );
}

#[test]
fn parse_synonym_guarantees() {
    assert_eq!(
        parse_stmt("guarantees that balance decreases by amount"),
        Rule::promise_stmt
    );
}

#[test]
fn parse_synonym_must_always() {
    assert_eq!(
        parse_stmt("must always satisfy balance >= 0"),
        Rule::promise_stmt
    );
}

#[test]
fn parse_synonym_ensure() {
    assert_eq!(
        parse_stmt("ensure sender is active\n  otherwise raise InvalidUserError"),
        Rule::check_stmt
    );
}

#[test]
fn parse_synonym_look_up() {
    assert_eq!(
        parse_stmt("look up user from database where id matches user_id"),
        Rule::fetch_stmt
    );
}

#[test]
fn parse_synonym_atomically() {
    assert_eq!(
        parse_stmt(
            "atomically\n  update User in database where id is sender.id set balance = new_sender_balance\n  update User in database where id is receiver.id set balance = new_receiver_balance"
        ),
        Rule::together_stmt
    );
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn parse_check_no_otherwise() {
    assert_eq!(
        parse_stmt("check sender.balance >= 0"),
        Rule::check_stmt
    );
}

#[test]
fn parse_nested_do_with_children() {
    // Two levels of do nesting: outer do + inner check child
    parse_file(
        "do transfer money safely\n  from sender:User, receiver:User, amount:PositiveAmount\n  -> TransferResult\n\n  check sender.balance >= amount\n    otherwise raise InsufficientBalanceError",
    );
}

#[test]
fn parse_full_wallet_fixture_file() {
    // Complete multi-pattern file — ultimate acceptance test.
    // Covers define, describe, error, do, promise, check, let, update, return
    // in a single coherent document.
    parse_file(
        r#"-- types
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
    with sender = sender, receiver = receiver, amount = amount"#,
    );
}
