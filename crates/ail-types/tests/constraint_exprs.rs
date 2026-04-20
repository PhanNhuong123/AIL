use ail_types::{
    parse_constraint_expr, parse_value_expr, ArithOp, CompareOp, ConstraintExpr, LiteralValue,
    ParseError, ValueExpr,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_c(s: &str) -> ConstraintExpr {
    parse_constraint_expr(s).unwrap_or_else(|e| panic!("parse_constraint_expr({s:?}) failed: {e}"))
}

fn parse_v(s: &str) -> ValueExpr {
    parse_value_expr(s).unwrap_or_else(|e| panic!("parse_value_expr({s:?}) failed: {e}"))
}

/// Assert parse → display → parse produces the same AST (roundtrip).
fn roundtrip_constraint(s: &str) {
    let ast = parse_c(s);
    let rendered = ast.to_string();
    let ast2 = parse_constraint_expr(&rendered)
        .unwrap_or_else(|e| panic!("re-parse of {rendered:?} failed: {e}"));
    assert_eq!(
        ast, ast2,
        "roundtrip failed for {s:?}: rendered as {rendered:?}"
    );
}

fn roundtrip_value(s: &str) {
    let ast = parse_v(s);
    let rendered = ast.to_string();
    let ast2 = parse_value_expr(&rendered)
        .unwrap_or_else(|e| panic!("re-parse of {rendered:?} failed: {e}"));
    assert_eq!(
        ast, ast2,
        "roundtrip failed for {s:?}: rendered as {rendered:?}"
    );
}

// ── Literal tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_literal_integer() {
    assert_eq!(parse_v("42"), ValueExpr::Literal(LiteralValue::Integer(42)));
}

#[test]
fn parse_literal_float() {
    assert_eq!(parse_v("2.5"), ValueExpr::Literal(LiteralValue::Float(2.5)));
}

#[test]
fn parse_literal_text() {
    assert_eq!(
        parse_v("\"hello\""),
        ValueExpr::Literal(LiteralValue::Text("hello".to_owned()))
    );
}

#[test]
fn parse_literal_bool() {
    assert_eq!(
        parse_v("true"),
        ValueExpr::Literal(LiteralValue::Bool(true))
    );
    assert_eq!(
        parse_v("false"),
        ValueExpr::Literal(LiteralValue::Bool(false))
    );
}

#[test]
fn parse_literal_nothing() {
    assert_eq!(
        parse_v("nothing"),
        ValueExpr::Literal(LiteralValue::Nothing)
    );
}

// ── Ref tests ─────────────────────────────────────────────────────────────────

#[test]
fn parse_ref_simple_identifier() {
    assert_eq!(
        parse_v("balance"),
        ValueExpr::Ref(vec!["balance".to_owned()])
    );
}

#[test]
fn parse_ref_with_field_access() {
    assert_eq!(
        parse_v("sender.balance"),
        ValueExpr::Ref(vec!["sender".to_owned(), "balance".to_owned()])
    );
    // Three levels
    assert_eq!(
        parse_v("result.sender.balance"),
        ValueExpr::Ref(vec![
            "result".to_owned(),
            "sender".to_owned(),
            "balance".to_owned()
        ])
    );
}

// ── Old tests ─────────────────────────────────────────────────────────────────

#[test]
fn parse_old_expression() {
    let expected = ValueExpr::Old(Box::new(ValueExpr::Ref(vec![
        "sender".to_owned(),
        "balance".to_owned(),
    ])));
    assert_eq!(parse_v("old(sender.balance)"), expected);
}

// ── Call tests ────────────────────────────────────────────────────────────────

#[test]
fn parse_call_with_single_arg() {
    let expected = ValueExpr::Call {
        name: "len".to_owned(),
        args: vec![ValueExpr::Ref(vec!["items".to_owned()])],
    };
    assert_eq!(parse_v("len(items)"), expected);
}

// ── Arithmetic tests ──────────────────────────────────────────────────────────

#[test]
fn parse_arithmetic_addition() {
    let expected = ValueExpr::Arithmetic {
        op: ArithOp::Add,
        left: Box::new(ValueExpr::Ref(vec!["a".to_owned()])),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(1))),
    };
    assert_eq!(parse_v("a + 1"), expected);
}

#[test]
fn parse_arithmetic_subtraction_with_old() {
    // old(sender.balance) - amount
    let expected = ValueExpr::Arithmetic {
        op: ArithOp::Sub,
        left: Box::new(ValueExpr::Old(Box::new(ValueExpr::Ref(vec![
            "sender".to_owned(),
            "balance".to_owned(),
        ])))),
        right: Box::new(ValueExpr::Ref(vec!["amount".to_owned()])),
    };
    assert_eq!(parse_v("old(sender.balance) - amount"), expected);
}

#[test]
fn parse_arithmetic_operator_precedence() {
    // a + b * c  →  Add(a, Mul(b, c))
    let result = parse_v("a + b * c");
    assert_eq!(
        result,
        ValueExpr::Arithmetic {
            op: ArithOp::Add,
            left: Box::new(ValueExpr::Ref(vec!["a".to_owned()])),
            right: Box::new(ValueExpr::Arithmetic {
                op: ArithOp::Mul,
                left: Box::new(ValueExpr::Ref(vec!["b".to_owned()])),
                right: Box::new(ValueExpr::Ref(vec!["c".to_owned()])),
            }),
        }
    );
}

// ── Compare tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_compare_gte() {
    let ast = parse_c("sender.balance >= 0");
    assert_eq!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::Gte,
            left: Box::new(ValueExpr::Ref(vec![
                "sender".to_owned(),
                "balance".to_owned()
            ])),
            right: Box::new(ValueExpr::Literal(LiteralValue::Integer(0))),
        }
    );
}

#[test]
fn parse_compare_lte() {
    let ast = parse_c("x <= 100");
    assert!(matches!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::Lte,
            ..
        }
    ));
}

#[test]
fn parse_compare_is_equality() {
    let ast = parse_c("sender.status is \"active\"");
    assert_eq!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::Is,
            left: Box::new(ValueExpr::Ref(vec![
                "sender".to_owned(),
                "status".to_owned()
            ])),
            right: Box::new(ValueExpr::Literal(LiteralValue::Text("active".to_owned()))),
        }
    );
}

#[test]
fn parse_compare_is_not_inequality() {
    let ast = parse_c("user.email is not nothing");
    assert_eq!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::IsNot,
            left: Box::new(ValueExpr::Ref(vec!["user".to_owned(), "email".to_owned()])),
            right: Box::new(ValueExpr::Literal(LiteralValue::Nothing)),
        }
    );
}

#[test]
fn parse_compare_eq_double_equals() {
    let ast = parse_c("x == 42");
    assert!(matches!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::Eq,
            ..
        }
    ));
}

#[test]
fn parse_compare_neq_bang_equals() {
    let ast = parse_c("x != 0");
    assert!(matches!(
        ast,
        ConstraintExpr::Compare {
            op: CompareOp::Neq,
            ..
        }
    ));
}

// ── In tests ──────────────────────────────────────────────────────────────────

#[test]
fn parse_in_brace_literal_set() {
    let ast = parse_c("status in {\"active\", \"pending\"}");
    assert_eq!(
        ast,
        ConstraintExpr::In {
            value: Box::new(ValueExpr::Ref(vec!["status".to_owned()])),
            collection: Box::new(ValueExpr::Set(vec![
                ValueExpr::Literal(LiteralValue::Text("active".to_owned())),
                ValueExpr::Literal(LiteralValue::Text("pending".to_owned())),
            ])),
        }
    );
}

// ── Matches tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_matches_regex_pattern() {
    let ast = parse_c("code matches /^[A-Z]+$/");
    assert_eq!(
        ast,
        ConstraintExpr::Matches {
            value: Box::new(ValueExpr::Ref(vec!["code".to_owned()])),
            pattern: "^[A-Z]+$".to_owned(),
        }
    );
}

// ── Logical tests ─────────────────────────────────────────────────────────────

#[test]
fn parse_and_nary_flattening() {
    // a > 0 and b > 0 and c > 0  →  And([a>0, b>0, c>0])
    let ast = parse_c("a > 0 and b > 0 and c > 0");
    match ast {
        ConstraintExpr::And(terms) => assert_eq!(terms.len(), 3),
        other => panic!("expected And, got {other:?}"),
    }
}

#[test]
fn parse_or_nary_flattening() {
    let ast = parse_c("a > 0 or b > 0 or c > 0");
    match ast {
        ConstraintExpr::Or(terms) => assert_eq!(terms.len(), 3),
        other => panic!("expected Or, got {other:?}"),
    }
}

#[test]
fn parse_not_negation() {
    let ast = parse_c("not x > 0");
    assert!(matches!(ast, ConstraintExpr::Not(_)));
}

// ── Quantifier tests ──────────────────────────────────────────────────────────

#[test]
fn parse_forall_quantifier() {
    let ast = parse_c("for all item in order.items, item.price > 0");
    match ast {
        ConstraintExpr::ForAll { variable, .. } => {
            assert_eq!(variable, "item");
        }
        other => panic!("expected ForAll, got {other:?}"),
    }
}

#[test]
fn parse_exists_quantifier() {
    let ast = parse_c("exists r in results where r.status is \"ok\"");
    match ast {
        ConstraintExpr::Exists { variable, .. } => {
            assert_eq!(variable, "r");
        }
        other => panic!("expected Exists, got {other:?}"),
    }
}

// ── Roundtrip tests ───────────────────────────────────────────────────────────

#[test]
fn roundtrip_wallet_pre_contract() {
    roundtrip_constraint("sender.status is \"active\"");
}

#[test]
fn roundtrip_wallet_post_contract() {
    // result.sender.balance is old(sender.balance) - amount
    roundtrip_constraint("result.sender.balance is old(sender.balance) - amount");
}

#[test]
fn roundtrip_value_field_access() {
    roundtrip_value("sender.balance");
}

#[test]
fn roundtrip_value_arithmetic() {
    roundtrip_value("old(sender.balance) - amount");
}

// ── Serde roundtrip tests ─────────────────────────────────────────────────────

#[test]
fn serde_roundtrip_constraint_expr() {
    let ast = parse_c("sender.balance >= 0 and sender.status is \"active\"");
    let json = serde_json::to_string(&ast).expect("serialize");
    let restored: ConstraintExpr = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ast, restored);
}

#[test]
fn serde_roundtrip_value_expr() {
    let ast = parse_v("old(sender.balance) - amount");
    let json = serde_json::to_string(&ast).expect("serialize");
    let restored: ValueExpr = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ast, restored);
}

// ── Error tests ───────────────────────────────────────────────────────────────

#[test]
fn parse_error_unexpected_char() {
    let err = parse_constraint_expr("sender.balance >= @").unwrap_err();
    assert!(
        matches!(err, ParseError::UnexpectedChar('@', _)),
        "expected UnexpectedChar('@', _), got {err:?}"
    );
}

#[test]
fn parse_error_unexpected_eof() {
    // Partial expression: `sender.balance >=` — nothing after the operator
    let err = parse_constraint_expr("sender.balance >=").unwrap_err();
    assert!(
        matches!(err, ParseError::UnexpectedEof | ParseError::Expected(..)),
        "expected UnexpectedEof or Expected, got {err:?}"
    );
}

#[test]
fn parse_error_unterminated_string() {
    let err = parse_constraint_expr("sender.status is \"active").unwrap_err();
    assert!(
        matches!(err, ParseError::UnterminatedString),
        "expected UnterminatedString, got {err:?}"
    );
}

#[test]
fn parse_error_unterminated_regex() {
    let err = parse_constraint_expr("code matches /^abc").unwrap_err();
    assert!(
        matches!(err, ParseError::UnterminatedRegex),
        "expected UnterminatedRegex, got {err:?}"
    );
}
