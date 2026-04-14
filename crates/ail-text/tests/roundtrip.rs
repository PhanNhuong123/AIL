use ail_text::{parse, parse_directory, render};
use std::path::Path;

/// Parse → render at full depth.
fn roundtrip(input: &str) -> String {
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));
    render(&graph, usize::MAX)
}

/// Parse → render → parse → render at full depth. Returns both rendered outputs.
fn double_roundtrip(input: &str) -> (String, String) {
    let first = roundtrip(input);
    let second = roundtrip(&first);
    (first, second)
}

/// Read fixture file content, normalizing Windows CRLF to LF.
fn read_fixture(name: &str) -> String {
    let path = format!("tests/fixtures/wallet_full/{name}");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
        .replace("\r\n", "\n")
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 1: Per-fixture-file identity
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_fixture_positive_amount() {
    let content = read_fixture("positive_amount.ail");
    let output = roundtrip(&content);
    assert_eq!(output, content, "positive_amount.ail did not roundtrip identically");
}

#[test]
fn roundtrip_fixture_transfer_money() {
    let content = read_fixture("transfer_money.ail");
    let output = roundtrip(&content);
    assert_eq!(output, content, "transfer_money.ail did not roundtrip identically");
}

#[test]
fn roundtrip_fixture_transfer_result() {
    let content = read_fixture("transfer_result.ail");
    let output = roundtrip(&content);
    assert_eq!(output, content, "transfer_result.ail did not roundtrip identically");
}

#[test]
fn roundtrip_fixture_user() {
    let content = read_fixture("user.ail");
    let output = roundtrip(&content);
    assert_eq!(output, content, "user.ail did not roundtrip identically");
}

#[test]
fn roundtrip_fixture_wallet_balance() {
    let content = read_fixture("wallet_balance.ail");
    let output = roundtrip(&content);
    assert_eq!(output, content, "wallet_balance.ail did not roundtrip identically");
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 2: Idempotency — double roundtrip
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_double_all_fixtures() {
    let fixtures = [
        "positive_amount.ail",
        "transfer_money.ail",
        "transfer_result.ail",
        "user.ail",
        "wallet_balance.ail",
    ];

    for name in fixtures {
        let content = read_fixture(name);
        let (first, second) = double_roundtrip(&content);
        assert_eq!(
            first, second,
            "{name}: double roundtrip produced different output on second pass"
        );
    }
}

#[test]
fn roundtrip_directory_render_is_idempotent() {
    let fixture_dir = Path::new("tests/fixtures/wallet_full");
    if !fixture_dir.exists() {
        panic!("fixture directory not found: {}", fixture_dir.display());
    }

    let graph =
        parse_directory(fixture_dir).unwrap_or_else(|e| panic!("parse_directory failed: {e}"));
    let first = render(&graph, usize::MAX);

    let graph2 = parse(&first).unwrap_or_else(|e| {
        panic!("re-parse of rendered directory output failed: {e}")
    });
    let second = render(&graph2, usize::MAX);

    assert_eq!(
        first, second,
        "directory roundtrip: render is not idempotent"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 3: All-pattern combined identity
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_all_patterns_combined() {
    // All 17 patterns in a realistic arrangement.
    // `together` is last child of `do` because its grammar greedily captures
    // subsequent cont2 lines as inline children. `retry` is top-level.
    let source = "\
define WalletBalance:number where value >= 0

define UserId:text

describe User as
  id:UserId, balance:WalletBalance

describe TransferResult as
  sender:User, receiver:User, amount:WalletBalance

error InsufficientBalanceError
  carries current_balance:WalletBalance, requested_amount:WalletBalance

do transfer money safely
  from sender:User, receiver:User, amount:WalletBalance
  -> TransferResult
  or InsufficientBalanceError

  promise before: sender.balance >= amount
  promise before: amount > 0
  promise after: sender.balance >= 0

  check sender.balance >= amount
    otherwise raise InsufficientBalanceError carries current_balance = sender.balance, requested_amount = amount
  let new_balance:WalletBalance = sender.balance - amount
  fetch sender:User from database where id is sender.id
  save notification to queue with user_id = sender.id, message = transfer_complete
  update User in database where id is sender.id set balance = new_balance
  remove Session from store where token is expired_token
  for each item:User in users do validate balance
  match sender.status
    when \"active\": proceed
    when \"suspended\": raise InsufficientBalanceError
    otherwise: raise InsufficientBalanceError
  return TransferResult
    with sender = sender, receiver = receiver, amount = amount
  raise InsufficientBalanceError
    carries current_balance = sender.balance, requested_amount = amount
  together
    update User in database where id is sender.id set balance = x
    update User in database where id is receiver.id set balance = y

retry 3 times with delay 1 second
  fetch rate:WalletBalance from external_api where currency is \"USD\"
";

    let output = roundtrip(source);
    assert_eq!(output, source, "all-pattern combined source did not roundtrip identically");

    // Also verify double roundtrip stability
    let second = roundtrip(&output);
    assert_eq!(output, second, "all-pattern combined: not idempotent after second pass");
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 4: Synonym normalization stability
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_synonym_requires_normalizes() {
    // `requires` is a synonym for `promise before:`
    let input = "\
do validate user
  from user_id:UserId
  -> User

  requires that user_id is not empty
";

    let canonical = roundtrip(input);
    // Synonym should normalize to canonical form
    assert!(
        canonical.contains("promise before:"),
        "expected `requires` to normalize to `promise before:`, got:\n{canonical}"
    );
    assert!(
        !canonical.contains("requires"),
        "synonym `requires` should not appear in canonical output"
    );

    // Re-parse canonical → re-render: must be stable
    let re_rendered = roundtrip(&canonical);
    assert_eq!(
        canonical, re_rendered,
        "canonical form after synonym normalization is not stable"
    );
}

#[test]
fn roundtrip_synonym_guarantees_normalizes() {
    // `guarantees` is a synonym for `promise after:`
    let input = "\
do process payment
  from amount:WalletBalance
  -> WalletBalance

  guarantees that amount >= 0
";

    let canonical = roundtrip(input);
    assert!(
        canonical.contains("promise after:"),
        "expected `guarantees` to normalize to `promise after:`, got:\n{canonical}"
    );
    assert!(
        !canonical.contains("guarantees"),
        "synonym `guarantees` should not appear in canonical output"
    );

    let re_rendered = roundtrip(&canonical);
    assert_eq!(
        canonical, re_rendered,
        "canonical form after synonym normalization is not stable"
    );
}

#[test]
fn roundtrip_synonym_look_up_normalizes() {
    // `look up` is a synonym for `fetch`
    let input = "look up sender:User from database where id is sender_id\n";

    let canonical = roundtrip(input);
    assert!(
        canonical.contains("fetch sender:User"),
        "expected `look up` to normalize to `fetch`, got:\n{canonical}"
    );

    let re_rendered = roundtrip(&canonical);
    assert_eq!(
        canonical, re_rendered,
        "canonical form after synonym normalization is not stable"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Section 5: Lossy roundtrip documentation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn roundtrip_comments_are_lossy() {
    // Standalone comment lines are consumed by pest's implicit COMMENT rule
    // (silent) and do not survive roundtrip. This test documents that behavior.
    let with_comment = "\
-- this is a standalone comment
define WalletBalance:number where value >= 0
";

    let without_comment = "define WalletBalance:number where value >= 0\n";

    let output = roundtrip(with_comment);
    assert_eq!(
        output, without_comment,
        "standalone comment should be stripped during roundtrip"
    );

    // The canonical (no-comment) form roundtrips identically
    let re_rendered = roundtrip(&output);
    assert_eq!(output, re_rendered, "canonical form without comment is not stable");
}
