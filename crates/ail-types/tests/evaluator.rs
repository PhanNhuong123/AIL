use std::collections::HashMap;

use ail_types::{
    EvalContext, EvalError, Value,
    eval_constraint, eval_value,
    parse_constraint_expr, parse_value_expr,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn empty_ctx() -> EvalContext {
    EvalContext::new(HashMap::new())
}

fn ctx_with(pairs: &[(&str, Value)]) -> EvalContext {
    let bindings = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    EvalContext::new(bindings)
}

fn ctx_with_old(current: &[(&str, Value)], old: &[(&str, Value)]) -> EvalContext {
    let cur = current.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let old = old.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    EvalContext::with_old(cur, old)
}

fn eval_c(expr: &str, ctx: &EvalContext) -> Result<bool, EvalError> {
    let e = parse_constraint_expr(expr).expect("parse failed");
    eval_constraint(&e, ctx)
}

fn eval_v(expr: &str, ctx: &EvalContext) -> Result<Value, EvalError> {
    let e = parse_value_expr(expr).expect("parse failed");
    eval_value(&e, ctx)
}

fn record(fields: &[(&str, Value)]) -> Value {
    Value::Record(
        fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
    )
}

fn list(items: Vec<Value>) -> Value {
    Value::List(items)
}

// ── Literal evaluation ────────────────────────────────────────────────────────

#[test]
fn eval_literal_integer() {
    assert!(eval_c("42 == 42", &empty_ctx()).unwrap());
    assert!(!eval_c("42 == 43", &empty_ctx()).unwrap());
}

#[test]
fn eval_literal_float() {
    assert!(eval_c("3.14 == 3.14", &empty_ctx()).unwrap());
    assert!(!eval_c("3.14 == 3.15", &empty_ctx()).unwrap());
}

#[test]
fn eval_literal_text() {
    assert!(eval_c("\"hello\" == \"hello\"", &empty_ctx()).unwrap());
    assert!(!eval_c("\"hello\" == \"world\"", &empty_ctx()).unwrap());
}

#[test]
fn eval_literal_bool() {
    assert!(eval_c("true is true", &empty_ctx()).unwrap());
    assert!(eval_c("false is false", &empty_ctx()).unwrap());
    assert!(!eval_c("true is false", &empty_ctx()).unwrap());
}

#[test]
fn eval_literal_nothing() {
    let ctx = ctx_with(&[("x", Value::Nothing)]);
    assert!(eval_c("x is nothing", &ctx).unwrap());
    assert!(!eval_c("x is not nothing", &ctx).unwrap());
}

// ── Ref / field access ────────────────────────────────────────────────────────

#[test]
fn eval_ref_simple() {
    let ctx = ctx_with(&[("balance", Value::Integer(100))]);
    assert!(eval_c("balance >= 0", &ctx).unwrap());
    assert!(!eval_c("balance < 0", &ctx).unwrap());
}

#[test]
fn eval_ref_field_access() {
    let sender = record(&[("balance", Value::Integer(200))]);
    let ctx = ctx_with(&[("sender", sender)]);
    assert!(eval_c("sender.balance >= 0", &ctx).unwrap());
    assert!(eval_c("sender.balance == 200", &ctx).unwrap());
}

#[test]
fn eval_ref_deep_field_access() {
    let inner = record(&[("c", Value::Integer(42))]);
    let mid = record(&[("b", inner)]);
    let root = record(&[("a", mid)]);
    let ctx = ctx_with(&[("root", root)]);
    assert!(eval_c("root.a.b.c == 42", &ctx).unwrap());
}

#[test]
fn eval_ref_undefined_variable() {
    let ctx = empty_ctx();
    let result = eval_c("missing >= 0", &ctx);
    assert!(matches!(result, Err(EvalError::UndefinedVariable(name)) if name == "missing"));
}

#[test]
fn eval_ref_undefined_field() {
    let sender = record(&[("balance", Value::Integer(100))]);
    let ctx = ctx_with(&[("sender", sender)]);
    let result = eval_c("sender.nonexistent >= 0", &ctx);
    assert!(
        matches!(result, Err(EvalError::UndefinedField { field, .. }) if field == "nonexistent")
    );
}

#[test]
fn eval_ref_field_on_scalar() {
    let ctx = ctx_with(&[("balance", Value::Integer(100))]);
    let result = eval_c("balance.subfield == 0", &ctx);
    assert!(matches!(result, Err(EvalError::UndefinedField { .. })));
}

// ── old() snapshots ───────────────────────────────────────────────────────────

#[test]
fn eval_old_snapshot() {
    let ctx = ctx_with_old(
        &[("balance", Value::Integer(80))],
        &[("balance", Value::Integer(100))],
    );
    // current balance is 80, old balance is 100
    assert!(eval_c("balance == 80", &ctx).unwrap());
    assert!(eval_c("old(balance) == 100", &ctx).unwrap());
}

#[test]
fn eval_old_field_access() {
    let old_sender = record(&[("balance", Value::Integer(50))]);
    let new_sender = record(&[("balance", Value::Integer(30))]);
    let ctx = ctx_with_old(
        &[("sender", new_sender)],
        &[("sender", old_sender)],
    );
    assert!(eval_c("old(sender.balance) == 50", &ctx).unwrap());
    assert!(eval_c("sender.balance == 30", &ctx).unwrap());
}

#[test]
fn eval_old_outside_context() {
    let ctx = ctx_with(&[("balance", Value::Integer(100))]);
    let result = eval_c("old(balance) == 100", &ctx);
    assert!(matches!(result, Err(EvalError::OldOutsidePostCondition)));
}

// ── Call: len ─────────────────────────────────────────────────────────────────

#[test]
fn eval_call_len_list() {
    let ctx = ctx_with(&[(
        "items",
        list(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]),
    )]);
    assert!(eval_c("len(items) == 3", &ctx).unwrap());
}

#[test]
fn eval_call_len_text() {
    let ctx = ctx_with(&[("name", Value::Text("hello".into()))]);
    assert!(eval_c("len(name) == 5", &ctx).unwrap());
}

#[test]
fn eval_call_unknown_function() {
    let result = eval_v("unknown_fn(42)", &empty_ctx());
    assert!(matches!(result, Err(EvalError::UnknownFunction(name)) if name == "unknown_fn"));
}

#[test]
fn eval_call_wrong_arg_count() {
    // len requires exactly 1 arg; pass 0 by building the AST manually
    use ail_types::{ValueExpr, eval_value};
    let expr = ValueExpr::Call { name: "len".into(), args: vec![] };
    let result = eval_value(&expr, &empty_ctx());
    assert!(
        matches!(result, Err(EvalError::WrongArgCount { name, expected: 1, actual: 0 }) if name == "len")
    );
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

#[test]
fn eval_arithmetic_all_ops() {
    assert!(eval_c("1 + 2 == 3", &empty_ctx()).unwrap());
    assert!(eval_c("5 - 3 == 2", &empty_ctx()).unwrap());
    assert!(eval_c("2 * 3 == 6", &empty_ctx()).unwrap());
    assert!(eval_c("6 / 2 == 3", &empty_ctx()).unwrap());
    assert!(eval_c("7 % 3 == 1", &empty_ctx()).unwrap());
}

#[test]
fn eval_arithmetic_float_coercion() {
    // integer + float → float
    assert!(eval_c("1 + 0.5 == 1.5", &empty_ctx()).unwrap());
    assert!(eval_c("2.0 * 3 == 6.0", &empty_ctx()).unwrap());
}

#[test]
fn eval_arithmetic_division_by_zero() {
    let result = eval_c("10 / 0 == 0", &empty_ctx());
    assert!(matches!(result, Err(EvalError::DivisionByZero)));
}

#[test]
fn eval_arithmetic_mod_zero() {
    let result = eval_c("10 % 0 == 0", &empty_ctx());
    assert!(matches!(result, Err(EvalError::DivisionByZero)));
}

// ── Compare ───────────────────────────────────────────────────────────────────

#[test]
fn eval_compare_all_ops() {
    assert!(eval_c("3 >= 2", &empty_ctx()).unwrap());
    assert!(eval_c("3 >= 3", &empty_ctx()).unwrap());
    assert!(!eval_c("2 >= 3", &empty_ctx()).unwrap());

    assert!(eval_c("2 <= 3", &empty_ctx()).unwrap());
    assert!(eval_c("3 <= 3", &empty_ctx()).unwrap());
    assert!(!eval_c("3 <= 2", &empty_ctx()).unwrap());

    assert!(eval_c("3 > 2", &empty_ctx()).unwrap());
    assert!(!eval_c("2 > 3", &empty_ctx()).unwrap());

    assert!(eval_c("2 < 3", &empty_ctx()).unwrap());
    assert!(!eval_c("3 < 2", &empty_ctx()).unwrap());

    assert!(eval_c("2 == 2", &empty_ctx()).unwrap());
    assert!(!eval_c("2 == 3", &empty_ctx()).unwrap());

    assert!(eval_c("2 != 3", &empty_ctx()).unwrap());
    assert!(!eval_c("2 != 2", &empty_ctx()).unwrap());
}

#[test]
fn eval_compare_is_eq_equivalent() {
    let ctx = ctx_with(&[("status", Value::Text("active".into()))]);
    let via_is = eval_c("status is \"active\"", &ctx).unwrap();
    let via_eq = eval_c("status == \"active\"", &ctx).unwrap();
    assert_eq!(via_is, via_eq);
    assert!(via_is);
}

#[test]
fn eval_compare_nothing_ordering_is_error() {
    // nothing > 5 must return TypeMismatch, not a boolean
    use ail_types::{ConstraintExpr, CompareOp, ValueExpr, LiteralValue};
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Gt,
        left: Box::new(ValueExpr::Literal(LiteralValue::Nothing)),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(5))),
    };
    let result = eval_constraint(&expr, &empty_ctx());
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── In / Matches ──────────────────────────────────────────────────────────────

#[test]
fn eval_in_member() {
    let ctx = ctx_with(&[("status", Value::Text("active".into()))]);
    assert!(eval_c("status in {\"active\", \"pending\"}", &ctx).unwrap());
}

#[test]
fn eval_in_non_member() {
    let ctx = ctx_with(&[("status", Value::Text("closed".into()))]);
    assert!(!eval_c("status in {\"active\", \"pending\"}", &ctx).unwrap());
}

#[test]
fn eval_matches_valid() {
    let ctx = ctx_with(&[("email", Value::Text("user@example.com".into()))]);
    assert!(eval_c("email matches /.*@.*/", &ctx).unwrap());
    assert!(!eval_c("email matches /^[0-9]+$/", &ctx).unwrap());
}

#[test]
fn eval_matches_invalid_regex() {
    let ctx = ctx_with(&[("val", Value::Text("hello".into()))]);
    let result = eval_c("val matches /[invalid/", &ctx);
    assert!(matches!(result, Err(EvalError::InvalidRegex(..))));
}

// ── Logical operators ─────────────────────────────────────────────────────────

#[test]
fn eval_and_short_circuit() {
    // false and undefined_var — should return false without evaluating the second operand
    let ctx = empty_ctx();
    // Build manually to ensure short-circuit: false AND (something that would error)
    use ail_types::{ConstraintExpr, CompareOp, ValueExpr, LiteralValue};
    let false_expr = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ValueExpr::Literal(LiteralValue::Integer(1))),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(2))),
    };
    let would_error = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ValueExpr::Ref(vec!["undefined_var".into()])),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(0))),
    };
    let and_expr = ConstraintExpr::And(vec![false_expr, would_error]);
    // Short-circuit: false AND _ = false (no error raised)
    assert!(!eval_constraint(&and_expr, &ctx).unwrap());
}

#[test]
fn eval_or_short_circuit() {
    // true or undefined_var — should return true without evaluating the second operand
    let ctx = empty_ctx();
    use ail_types::{ConstraintExpr, CompareOp, ValueExpr, LiteralValue};
    let true_expr = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ValueExpr::Literal(LiteralValue::Integer(1))),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(1))),
    };
    let would_error = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ValueExpr::Ref(vec!["undefined_var".into()])),
        right: Box::new(ValueExpr::Literal(LiteralValue::Integer(0))),
    };
    let or_expr = ConstraintExpr::Or(vec![true_expr, would_error]);
    assert!(eval_constraint(&or_expr, &ctx).unwrap());
}

#[test]
fn eval_not() {
    assert!(!eval_c("not true is true", &empty_ctx()).unwrap());
    assert!(eval_c("not 1 == 2", &empty_ctx()).unwrap());
}

// ── Quantifiers ───────────────────────────────────────────────────────────────

#[test]
fn eval_forall_all_pass() {
    let ctx = ctx_with(&[(
        "prices",
        list(vec![Value::Integer(10), Value::Integer(20), Value::Integer(5)]),
    )]);
    assert!(eval_c("for all p in prices, p > 0", &ctx).unwrap());
}

#[test]
fn eval_forall_one_fails() {
    let ctx = ctx_with(&[(
        "prices",
        list(vec![Value::Integer(10), Value::Integer(-1), Value::Integer(5)]),
    )]);
    assert!(!eval_c("for all p in prices, p > 0", &ctx).unwrap());
}

#[test]
fn eval_forall_empty_collection_vacuous_true() {
    let ctx = ctx_with(&[("items", list(vec![]))]);
    assert!(eval_c("for all x in items, x > 0", &ctx).unwrap());
}

#[test]
fn eval_exists_one_passes() {
    let ctx = ctx_with(&[(
        "scores",
        list(vec![Value::Integer(40), Value::Integer(90), Value::Integer(55)]),
    )]);
    assert!(eval_c("exists s in scores where s >= 90", &ctx).unwrap());
}

#[test]
fn eval_exists_none_pass() {
    let ctx = ctx_with(&[(
        "scores",
        list(vec![Value::Integer(40), Value::Integer(60), Value::Integer(55)]),
    )]);
    assert!(!eval_c("exists s in scores where s >= 90", &ctx).unwrap());
}

#[test]
fn eval_exists_empty_collection_vacuous_false() {
    let ctx = ctx_with(&[("items", list(vec![]))]);
    assert!(!eval_c("exists x in items where x > 0", &ctx).unwrap());
}

// ── Complex: wallet post-condition ────────────────────────────────────────────

#[test]
fn eval_complex_wallet_postcondition() {
    // Simulates: result.sender.balance == old(sender.balance) - amount
    // current: sender.balance = 80, amount = 20
    // old:     sender.balance = 100
    // Expected: 80 == 100 - 20  →  true
    let new_sender = record(&[("balance", Value::Integer(80))]);
    let result_val = record(&[("sender", new_sender)]);
    let old_sender = record(&[("balance", Value::Integer(100))]);

    let ctx = ctx_with_old(
        &[
            ("result", result_val),
            ("sender", record(&[("balance", Value::Integer(80))])),
            ("amount", Value::Integer(20)),
        ],
        &[("sender", old_sender)],
    );

    assert!(eval_c("result.sender.balance == old(sender.balance) - amount", &ctx).unwrap());
}
