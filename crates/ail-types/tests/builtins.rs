use ail_types::{BuiltinSemanticType, Value};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn int(n: i64) -> Value {
    Value::Integer(n)
}

fn float(f: f64) -> Value {
    Value::Float(f)
}

fn text(s: &str) -> Value {
    Value::Text(s.to_string())
}

fn valid(ty: BuiltinSemanticType, value: Value) -> bool {
    ty.validate_value(&value)
}

// ──────────────────────────────────────────────────────────────────────────────
// from_name lookup
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_from_name_returns_all_known_types() {
    let cases = [
        ("PositiveInteger", BuiltinSemanticType::PositiveInteger),
        (
            "NonNegativeInteger",
            BuiltinSemanticType::NonNegativeInteger,
        ),
        ("PositiveAmount", BuiltinSemanticType::PositiveAmount),
        ("Percentage", BuiltinSemanticType::Percentage),
        ("NonEmptyText", BuiltinSemanticType::NonEmptyText),
        ("EmailAddress", BuiltinSemanticType::EmailAddress),
        ("Identifier", BuiltinSemanticType::Identifier),
    ];
    for (name, expected) in cases {
        assert_eq!(
            BuiltinSemanticType::from_name(name),
            Some(expected),
            "from_name({name:?}) should return Some(...)"
        );
    }
}

#[test]
fn t023_from_name_returns_none_for_unknown() {
    assert_eq!(BuiltinSemanticType::from_name("integer"), None);
    assert_eq!(BuiltinSemanticType::from_name("text"), None);
    assert_eq!(BuiltinSemanticType::from_name("Amount"), None);
    assert_eq!(BuiltinSemanticType::from_name(""), None);
    assert_eq!(BuiltinSemanticType::from_name("positiveinteger"), None); // case-sensitive
}

#[test]
fn t023_name_roundtrips_through_from_name() {
    for &ty in BuiltinSemanticType::ALL {
        let roundtripped = BuiltinSemanticType::from_name(ty.name());
        assert_eq!(
            roundtripped,
            Some(ty),
            "name() → from_name() roundtrip failed for {:?}",
            ty
        );
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// base_type
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_base_type_numeric_types_are_integer_or_number() {
    assert_eq!(BuiltinSemanticType::PositiveInteger.base_type(), "integer");
    assert_eq!(
        BuiltinSemanticType::NonNegativeInteger.base_type(),
        "integer"
    );
    assert_eq!(BuiltinSemanticType::PositiveAmount.base_type(), "number");
    assert_eq!(BuiltinSemanticType::Percentage.base_type(), "number");
}

#[test]
fn t023_base_type_text_types_are_text() {
    assert_eq!(BuiltinSemanticType::NonEmptyText.base_type(), "text");
    assert_eq!(BuiltinSemanticType::EmailAddress.base_type(), "text");
    assert_eq!(BuiltinSemanticType::Identifier.base_type(), "text");
}

// ──────────────────────────────────────────────────────────────────────────────
// constraint_exprs
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_constraint_exprs_are_non_empty_for_all_types() {
    for &ty in BuiltinSemanticType::ALL {
        assert!(
            !ty.constraint_exprs().is_empty(),
            "{:?} must have at least one constraint expression",
            ty
        );
    }
}

#[test]
fn t023_percentage_has_two_constraint_exprs() {
    let exprs = BuiltinSemanticType::Percentage.constraint_exprs();
    assert_eq!(
        exprs.len(),
        2,
        "Percentage needs both lower and upper bound"
    );
    assert!(exprs.iter().any(|e| e.contains(">=")));
    assert!(exprs.iter().any(|e| e.contains("<=")));
}

// ──────────────────────────────────────────────────────────────────────────────
// PositiveInteger
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_positive_integer_valid() {
    let ty = BuiltinSemanticType::PositiveInteger;
    assert!(valid(ty, int(1))); // minimum valid
    assert!(valid(ty, int(100)));
    assert!(valid(ty, int(i64::MAX)));
}

#[test]
fn t023_positive_integer_boundary_zero_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveInteger, int(0)));
}

#[test]
fn t023_positive_integer_negative_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveInteger, int(-1)));
    assert!(!valid(BuiltinSemanticType::PositiveInteger, int(i64::MIN)));
}

#[test]
fn t023_positive_integer_wrong_kind_invalid() {
    let ty = BuiltinSemanticType::PositiveInteger;
    assert!(!valid(ty, float(1.0)));
    assert!(!valid(ty, text("1")));
    assert!(!valid(ty, Value::Bool(true)));
    assert!(!valid(ty, Value::Nothing));
}

// ──────────────────────────────────────────────────────────────────────────────
// NonNegativeInteger
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_non_negative_integer_valid() {
    let ty = BuiltinSemanticType::NonNegativeInteger;
    assert!(valid(ty, int(0))); // boundary — zero is valid
    assert!(valid(ty, int(1)));
    assert!(valid(ty, int(1_000_000)));
}

#[test]
fn t023_non_negative_integer_boundary_minus_one_invalid() {
    assert!(!valid(BuiltinSemanticType::NonNegativeInteger, int(-1)));
}

#[test]
fn t023_non_negative_integer_wrong_kind_invalid() {
    let ty = BuiltinSemanticType::NonNegativeInteger;
    assert!(!valid(ty, float(0.0)));
    assert!(!valid(ty, text("0")));
}

// ──────────────────────────────────────────────────────────────────────────────
// PositiveAmount
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_positive_amount_valid_integer() {
    let ty = BuiltinSemanticType::PositiveAmount;
    assert!(valid(ty, int(1)));
    assert!(valid(ty, int(9999)));
}

#[test]
fn t023_positive_amount_valid_float() {
    let ty = BuiltinSemanticType::PositiveAmount;
    assert!(valid(ty, float(0.01)));
    assert!(valid(ty, float(99.99)));
    assert!(valid(ty, float(1.0)));
}

#[test]
fn t023_positive_amount_zero_integer_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveAmount, int(0)));
}

#[test]
fn t023_positive_amount_zero_float_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveAmount, float(0.0)));
}

#[test]
fn t023_positive_amount_negative_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveAmount, int(-1)));
    assert!(!valid(BuiltinSemanticType::PositiveAmount, float(-0.01)));
}

#[test]
fn t023_positive_amount_nan_invalid() {
    assert!(!valid(BuiltinSemanticType::PositiveAmount, float(f64::NAN)));
}

#[test]
fn t023_positive_amount_infinity_invalid() {
    assert!(!valid(
        BuiltinSemanticType::PositiveAmount,
        float(f64::INFINITY)
    ));
}

// ──────────────────────────────────────────────────────────────────────────────
// Percentage
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_percentage_valid_integer_range() {
    let ty = BuiltinSemanticType::Percentage;
    assert!(valid(ty, int(0))); // lower bound
    assert!(valid(ty, int(50)));
    assert!(valid(ty, int(100))); // upper bound
}

#[test]
fn t023_percentage_valid_float_range() {
    let ty = BuiltinSemanticType::Percentage;
    assert!(valid(ty, float(0.0)));
    assert!(valid(ty, float(50.5)));
    assert!(valid(ty, float(100.0)));
}

#[test]
fn t023_percentage_integer_out_of_range_invalid() {
    assert!(!valid(BuiltinSemanticType::Percentage, int(-1)));
    assert!(!valid(BuiltinSemanticType::Percentage, int(101)));
}

#[test]
fn t023_percentage_float_just_over_100_invalid() {
    assert!(!valid(BuiltinSemanticType::Percentage, float(100.01)));
}

#[test]
fn t023_percentage_float_just_under_0_invalid() {
    assert!(!valid(BuiltinSemanticType::Percentage, float(-0.01)));
}

#[test]
fn t023_percentage_nan_invalid() {
    assert!(!valid(BuiltinSemanticType::Percentage, float(f64::NAN)));
}

// ──────────────────────────────────────────────────────────────────────────────
// NonEmptyText
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_non_empty_text_valid() {
    let ty = BuiltinSemanticType::NonEmptyText;
    assert!(valid(ty, text("hello")));
    assert!(valid(ty, text("x")));
    assert!(valid(ty, text("  hello  "))); // has non-whitespace content
}

#[test]
fn t023_non_empty_text_empty_string_invalid() {
    assert!(!valid(BuiltinSemanticType::NonEmptyText, text("")));
}

#[test]
fn t023_non_empty_text_whitespace_only_invalid() {
    // Whitespace-only strings are treated as empty (trim semantics)
    assert!(!valid(BuiltinSemanticType::NonEmptyText, text(" ")));
    assert!(!valid(BuiltinSemanticType::NonEmptyText, text("   ")));
    assert!(!valid(BuiltinSemanticType::NonEmptyText, text("\t\n")));
}

#[test]
fn t023_non_empty_text_wrong_kind_invalid() {
    let ty = BuiltinSemanticType::NonEmptyText;
    assert!(!valid(ty, int(1)));
    assert!(!valid(ty, Value::Bool(true)));
}

// ──────────────────────────────────────────────────────────────────────────────
// EmailAddress
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_email_valid() {
    let ty = BuiltinSemanticType::EmailAddress;
    assert!(valid(ty, text("user@example.com")));
    assert!(valid(ty, text("a.b+tag@sub.domain.org")));
    assert!(valid(ty, text("test123@company.io")));
}

#[test]
fn t023_email_missing_at_invalid() {
    assert!(!valid(
        BuiltinSemanticType::EmailAddress,
        text("notanemail")
    ));
    assert!(!valid(
        BuiltinSemanticType::EmailAddress,
        text("no-at-sign")
    ));
}

#[test]
fn t023_email_missing_dot_in_domain_invalid() {
    assert!(!valid(
        BuiltinSemanticType::EmailAddress,
        text("user@nodot")
    ));
    assert!(!valid(
        BuiltinSemanticType::EmailAddress,
        text("missing.dot@nodot")
    ));
}

#[test]
fn t023_email_empty_invalid() {
    assert!(!valid(BuiltinSemanticType::EmailAddress, text("")));
}

#[test]
fn t023_email_wrong_kind_invalid() {
    assert!(!valid(BuiltinSemanticType::EmailAddress, int(42)));
}

// ──────────────────────────────────────────────────────────────────────────────
// Identifier
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_identifier_valid() {
    let ty = BuiltinSemanticType::Identifier;
    assert!(valid(ty, text("transfer_money")));
    assert!(valid(ty, text("_private")));
    assert!(valid(ty, text("camelCase")));
    assert!(valid(ty, text("A1B2C3")));
    assert!(valid(ty, text("x")));
}

#[test]
fn t023_identifier_starts_with_digit_invalid() {
    assert!(!valid(BuiltinSemanticType::Identifier, text("1abc")));
    assert!(!valid(BuiltinSemanticType::Identifier, text("0")));
}

#[test]
fn t023_identifier_contains_hyphen_invalid() {
    assert!(!valid(BuiltinSemanticType::Identifier, text("my-var")));
}

#[test]
fn t023_identifier_contains_space_invalid() {
    assert!(!valid(BuiltinSemanticType::Identifier, text("my var")));
}

#[test]
fn t023_identifier_empty_invalid() {
    assert!(!valid(BuiltinSemanticType::Identifier, text("")));
}

#[test]
fn t023_identifier_wrong_kind_invalid() {
    assert!(!valid(BuiltinSemanticType::Identifier, int(1)));
}

// ──────────────────────────────────────────────────────────────────────────────
// ALL slice covers every variant
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn t023_all_slice_has_seven_entries() {
    assert_eq!(BuiltinSemanticType::ALL.len(), 7);
}

#[test]
fn t023_all_slice_contains_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for ty in BuiltinSemanticType::ALL {
        assert!(
            seen.insert(ty.name()),
            "duplicate entry in ALL: {}",
            ty.name()
        );
    }
}
