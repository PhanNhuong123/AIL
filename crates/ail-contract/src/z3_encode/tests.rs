/// Z3 encoding tests for task 3.2.
///
/// All tests require the `z3-verify` feature:
///   cargo test -p ail-contract --features z3-verify
///
/// Tests validate that encoding is structurally correct (SAT/UNSAT outcomes match
/// expectations) and that unsupported variants produce the right error variant.
use ail_types::{ArithOp, CompareOp, ConstraintExpr, BuiltinSemanticType, LiteralValue, ValueExpr};
use z3::{Config, Context, SatResult, Solver};

use crate::errors::EncodeError;

use super::{
    context::EncodeContext,
    encoder::{encode_constraint, encode_value_int, encode_value_real},
    type_constraints::encode_type_constraint,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_ctx() -> (Config, Context) {
    let cfg = Config::new();
    let ctx = Context::new(&cfg);
    (cfg, ctx)
}

fn ref_expr(path: &[&str]) -> ValueExpr {
    ValueExpr::Ref(path.iter().map(|s| s.to_string()).collect())
}

fn old_ref_expr(path: &[&str]) -> ValueExpr {
    ValueExpr::Old(Box::new(ref_expr(path)))
}

fn int_lit(n: i64) -> ValueExpr {
    ValueExpr::Literal(LiteralValue::Integer(n))
}

fn float_lit(f: f64) -> ValueExpr {
    ValueExpr::Literal(LiteralValue::Float(f))
}

fn sat_check(solver: &Solver) -> SatResult {
    solver.check()
}

// ── Literal encoding ──────────────────────────────────────────────────────────

#[test]
fn z3_encode_literal_integer_as_int() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let result = encode_value_int(&int_lit(42), &mut enc);
    assert!(result.is_ok(), "integer literal should encode as Int");
}

#[test]
fn z3_encode_literal_float_as_real() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let result = encode_value_real(&float_lit(3.14), &mut enc);
    assert!(result.is_ok(), "float literal should encode as Real");
}

// ── Variable look-ups ─────────────────────────────────────────────────────────

#[test]
fn z3_encode_ref_from_context() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Gt,
        left: Box::new(ref_expr(&["x"])),
        right: Box::new(int_lit(0)),
    };
    let encoded = encode_constraint(&expr, &enc).expect("Ref should resolve from context");
    solver.assert(&encoded);
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_old_ref_from_context() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_old_int_var("balance");

    let solver = Solver::new(&z3);
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Gte,
        left: Box::new(old_ref_expr(&["balance"])),
        right: Box::new(int_lit(0)),
    };
    let encoded =
        encode_constraint(&expr, &enc).expect("Old(Ref) should resolve from old_vars context");
    solver.assert(&encoded);
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

// ── Compare encodings ─────────────────────────────────────────────────────────

#[test]
fn z3_encode_compare_gte_sat() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    let gte = ConstraintExpr::Compare {
        op: CompareOp::Gte,
        left: Box::new(ref_expr(&["x"])),
        right: Box::new(int_lit(0)),
    };
    let gt = ConstraintExpr::Compare {
        op: CompareOp::Gt,
        left: Box::new(ref_expr(&["x"])),
        right: Box::new(int_lit(0)),
    };
    solver.assert(&encode_constraint(&gte, &enc).unwrap());
    solver.assert(&encode_constraint(&gt, &enc).unwrap());
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_compare_gte_unsat() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    // x >= 10 AND x <= 5 → UNSAT
    solver.assert(
        &encode_constraint(
            &ConstraintExpr::Compare {
                op: CompareOp::Gte,
                left: Box::new(ref_expr(&["x"])),
                right: Box::new(int_lit(10)),
            },
            &enc,
        )
        .unwrap(),
    );
    solver.assert(
        &encode_constraint(
            &ConstraintExpr::Compare {
                op: CompareOp::Lte,
                left: Box::new(ref_expr(&["x"])),
                right: Box::new(int_lit(5)),
            },
            &enc,
        )
        .unwrap(),
    );
    assert_eq!(sat_check(&solver), SatResult::Unsat);
}

// ── Boolean connectives ───────────────────────────────────────────────────────

#[test]
fn z3_encode_and_conjunction() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    // x >= 0 and x < 100
    let expr = ConstraintExpr::And(vec![
        ConstraintExpr::Compare {
            op: CompareOp::Gte,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(0)),
        },
        ConstraintExpr::Compare {
            op: CompareOp::Lt,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(100)),
        },
    ]);
    solver.assert(&encode_constraint(&expr, &enc).unwrap());
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_or_disjunction() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    // x > 5 or x < 0  — satisfiable (e.g. x = 10)
    let expr = ConstraintExpr::Or(vec![
        ConstraintExpr::Compare {
            op: CompareOp::Gt,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(5)),
        },
        ConstraintExpr::Compare {
            op: CompareOp::Lt,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(0)),
        },
    ]);
    solver.assert(&encode_constraint(&expr, &enc).unwrap());
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_not_negation() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    // not (x == 0) → x can be anything non-zero
    let expr = ConstraintExpr::Not(Box::new(ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ref_expr(&["x"])),
        right: Box::new(int_lit(0)),
    }));
    solver.assert(&encode_constraint(&expr, &enc).unwrap());
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

#[test]
fn z3_encode_arithmetic_sub() {
    // new_balance == old(balance) - amount
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("new_balance");
    enc.add_int_var("amount");
    enc.add_old_int_var("balance");

    let solver = Solver::new(&z3);
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ref_expr(&["new_balance"])),
        right: Box::new(ValueExpr::Arithmetic {
            op: ArithOp::Sub,
            left: Box::new(old_ref_expr(&["balance"])),
            right: Box::new(ref_expr(&["amount"])),
        }),
    };
    let encoded = encode_constraint(&expr, &enc).expect("arithmetic sub should encode");
    solver.assert(&encoded);
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_arithmetic_promotes_to_real() {
    // balance >= 0.0  — float literal promotes both sides to Real
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_real_var("balance");

    let solver = Solver::new(&z3);
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Gte,
        left: Box::new(ref_expr(&["balance"])),
        right: Box::new(float_lit(0.0)),
    };
    let encoded = encode_constraint(&expr, &enc).expect("float literal should promote to Real");
    solver.assert(&encoded);
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_contradictory_unsat() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("x");

    let solver = Solver::new(&z3);
    // x > 5 and x < 3 → UNSAT
    let expr = ConstraintExpr::And(vec![
        ConstraintExpr::Compare {
            op: CompareOp::Gt,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(5)),
        },
        ConstraintExpr::Compare {
            op: CompareOp::Lt,
            left: Box::new(ref_expr(&["x"])),
            right: Box::new(int_lit(3)),
        },
    ]);
    solver.assert(&encode_constraint(&expr, &enc).unwrap());
    assert_eq!(sat_check(&solver), SatResult::Unsat);
}

// ── Set membership ────────────────────────────────────────────────────────────

#[test]
fn z3_encode_in_literal_set() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_int_var("status_code");

    let solver = Solver::new(&z3);
    // status_code in {200, 201, 204}
    let expr = ConstraintExpr::In {
        value: Box::new(ref_expr(&["status_code"])),
        collection: Box::new(ValueExpr::Set(vec![
            int_lit(200),
            int_lit(201),
            int_lit(204),
        ])),
    };
    let encoded = encode_constraint(&expr, &enc).expect("In with literal Set should encode");
    solver.assert(&encoded);
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

// ── Unsupported variants ──────────────────────────────────────────────────────

#[test]
fn z3_encode_mod_on_real_unsupported() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    enc.add_real_var("x");

    // Mod with a float literal forces Real path → Mod-on-Real
    let expr = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ValueExpr::Arithmetic {
            op: ArithOp::Mod,
            left: Box::new(float_lit(5.0)),
            right: Box::new(float_lit(2.0)),
        }),
        right: Box::new(int_lit(0)),
    };
    let err = encode_constraint(&expr, &enc).unwrap_err();
    assert!(
        matches!(err, EncodeError::UnsupportedConstraint { variant: "Mod-on-Real" }),
        "expected UnsupportedConstraint(Mod-on-Real), got {err:?}"
    );
}

#[test]
fn z3_encode_nothing_literal_unsupported() {
    let (_cfg, z3) = make_ctx();
    let enc = EncodeContext::new(&z3);

    let expr = ConstraintExpr::Compare {
        op: CompareOp::Is,
        left: Box::new(ValueExpr::Literal(LiteralValue::Nothing)),
        right: Box::new(int_lit(0)),
    };
    let err = encode_constraint(&expr, &enc).unwrap_err();
    assert!(
        matches!(err, EncodeError::UnsupportedConstraint { variant: "Nothing" }),
        "expected UnsupportedConstraint(Nothing), got {err:?}"
    );
}

#[test]
fn z3_encode_unsupported_matches() {
    let (_cfg, z3) = make_ctx();
    let enc = EncodeContext::new(&z3);

    let expr = ConstraintExpr::Matches {
        value: Box::new(ref_expr(&["email"])),
        pattern: r"^[^@]+@[^@]+$".to_string(),
    };
    let err = encode_constraint(&expr, &enc).unwrap_err();
    assert!(
        matches!(err, EncodeError::UnsupportedConstraint { variant: "Matches" }),
        "expected UnsupportedConstraint(Matches), got {err:?}"
    );
}

#[test]
fn z3_encode_unsupported_forall() {
    let (_cfg, z3) = make_ctx();
    let enc = EncodeContext::new(&z3);

    let expr = ConstraintExpr::ForAll {
        variable: "item".to_string(),
        collection: Box::new(ref_expr(&["items"])),
        condition: Box::new(ConstraintExpr::Compare {
            op: CompareOp::Gt,
            left: Box::new(ref_expr(&["item"])),
            right: Box::new(int_lit(0)),
        }),
    };
    let err = encode_constraint(&expr, &enc).unwrap_err();
    assert!(
        matches!(err, EncodeError::UnsupportedConstraint { variant: "ForAll" }),
        "expected UnsupportedConstraint(ForAll), got {err:?}"
    );
}

// ── Type constraint encoding ──────────────────────────────────────────────────

#[test]
fn z3_encode_type_constraint_positive_int() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let x = enc.add_int_var("amount");

    let solver = Solver::new(&z3);
    let dyn_x = z3::ast::Dynamic::from_ast(&x);
    let assertions =
        encode_type_constraint(BuiltinSemanticType::PositiveInteger, &dyn_x, &z3).unwrap();
    for a in &assertions {
        solver.assert(a);
    }
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_type_constraint_nonneg_int() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let x = enc.add_int_var("count");

    // Adding count == 0 should still be SAT (non-negative includes 0)
    let solver = Solver::new(&z3);
    use z3::ast::Ast;
    let dyn_x = z3::ast::Dynamic::from_ast(&x);
    let assertions =
        encode_type_constraint(BuiltinSemanticType::NonNegativeInteger, &dyn_x, &z3).unwrap();
    for a in &assertions {
        solver.assert(a);
    }
    let zero = z3::ast::Int::from_i64(&z3, 0);
    solver.assert(&x._eq(&zero));
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_type_constraint_percentage() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let p = enc.add_real_var("rate");

    let solver = Solver::new(&z3);
    use z3::ast::Ast;
    let dyn_p = z3::ast::Dynamic::from_ast(&p);
    let assertions =
        encode_type_constraint(BuiltinSemanticType::Percentage, &dyn_p, &z3).unwrap();
    for a in &assertions {
        solver.assert(a);
    }
    // rate = 50.0 should be SAT
    let fifty = z3::ast::Real::from_real(&z3, 50, 1);
    solver.assert(&p._eq(&fifty));
    assert_eq!(sat_check(&solver), SatResult::Sat);
}

#[test]
fn z3_encode_type_constraint_text_unsupported() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);
    let v = enc.add_bool_var("email"); // sort doesn't matter — type check happens first

    let dyn_v = z3::ast::Dynamic::from_ast(&v);
    let err = encode_type_constraint(BuiltinSemanticType::NonEmptyText, &dyn_v, &z3).unwrap_err();
    assert!(
        matches!(err, EncodeError::UnsupportedConstraint { variant: "type-text" }),
        "expected UnsupportedConstraint(type-text), got {err:?}"
    );
}

// ── Flagship integration test ─────────────────────────────────────────────────

/// Full CIC packet scenario: wallet transfer postcondition.
///
/// Asserts type constraints, a precondition, and a postcondition, then checks
/// that a concrete satisfying assignment exists (SAT check, not a proof).
/// Proving that `new_balance >= 0` *necessarily* follows from the constraints
/// is task 3.3's job (UNSAT of the negation).
///
/// Constraints asserted:
///   - sender.balance >= 0      (NonNegativeInteger type constraint)
///   - amount > 0               (PositiveInteger type constraint)
///   - sender.balance >= amount (precondition fact)
///   - new_balance == old(sender.balance) - amount  (postcondition)
///   - new_balance >= 0         (the candidate property — part of the SAT witness)
#[test]
fn z3_encode_wallet_transfer_full_packet() {
    let (_cfg, z3) = make_ctx();
    let mut enc = EncodeContext::new(&z3);

    let balance = enc.add_int_var("sender.balance");
    let amount = enc.add_int_var("amount");
    let new_balance = enc.add_int_var("new_balance");
    let _old_balance = enc.add_old_int_var("sender.balance");

    let solver = Solver::new(&z3);
    use z3::ast::{Dynamic, Int};

    // Type constraints
    let dyn_balance = Dynamic::from_ast(&balance);
    let dyn_amount = Dynamic::from_ast(&amount);
    for a in encode_type_constraint(BuiltinSemanticType::NonNegativeInteger, &dyn_balance, &z3)
        .unwrap()
    {
        solver.assert(&a);
    }
    for a in
        encode_type_constraint(BuiltinSemanticType::PositiveInteger, &dyn_amount, &z3).unwrap()
    {
        solver.assert(&a);
    }

    // Precondition: sender.balance >= amount
    let pre = ConstraintExpr::Compare {
        op: CompareOp::Gte,
        left: Box::new(ref_expr(&["sender.balance"])),
        right: Box::new(ref_expr(&["amount"])),
    };
    solver.assert(&encode_constraint(&pre, &enc).unwrap());

    // Postcondition: new_balance == old(sender.balance) - amount
    let post = ConstraintExpr::Compare {
        op: CompareOp::Eq,
        left: Box::new(ref_expr(&["new_balance"])),
        right: Box::new(ValueExpr::Arithmetic {
            op: ArithOp::Sub,
            left: Box::new(old_ref_expr(&["sender.balance"])),
            right: Box::new(ref_expr(&["amount"])),
        }),
    };
    solver.assert(&encode_constraint(&post, &enc).unwrap());

    // Prove new_balance >= 0
    let zero = Int::from_i64(&z3, 0);
    solver.assert(&new_balance.ge(&zero));

    assert_eq!(
        sat_check(&solver),
        SatResult::Sat,
        "wallet transfer postcondition should be provably satisfiable"
    );
}
