use ail_types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};
use z3::ast::{Ast, Bool, Int, Real};

use crate::errors::EncodeError;

use super::context::EncodeContext;

// ── Public API ────────────────────────────────────────────────────────────────

/// Encode a [`ConstraintExpr`] into a Z3 [`Bool`] expression.
///
/// The caller must pre-register every variable path referenced by `expr` in `ctx`
/// before calling this function. Missing registrations produce
/// [`EncodeError::UnboundVariable`].
///
/// Unsupported variants (`Matches`, `ForAll`, `Exists`, `In` with a non-literal
/// collection) produce [`EncodeError::UnsupportedConstraint`].
pub fn encode_constraint<'ctx>(
    expr: &ConstraintExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    match expr {
        ConstraintExpr::Compare { op, left, right } => encode_compare(op, left, right, ctx),

        ConstraintExpr::And(children) => {
            let bools: Result<Vec<Bool<'ctx>>, EncodeError> =
                children.iter().map(|c| encode_constraint(c, ctx)).collect();
            let bools = bools?;
            let refs: Vec<&Bool<'ctx>> = bools.iter().collect();
            Ok(Bool::and(ctx.z3, &refs))
        }

        ConstraintExpr::Or(children) => {
            let bools: Result<Vec<Bool<'ctx>>, EncodeError> =
                children.iter().map(|c| encode_constraint(c, ctx)).collect();
            let bools = bools?;
            let refs: Vec<&Bool<'ctx>> = bools.iter().collect();
            Ok(Bool::or(ctx.z3, &refs))
        }

        ConstraintExpr::Not(inner) => Ok(encode_constraint(inner, ctx)?.not()),

        ConstraintExpr::In { value, collection } => encode_in(value, collection, ctx),

        ConstraintExpr::Matches { .. } => {
            Err(EncodeError::UnsupportedConstraint { variant: "Matches" })
        }
        ConstraintExpr::ForAll { .. } => {
            Err(EncodeError::UnsupportedConstraint { variant: "ForAll" })
        }
        ConstraintExpr::Exists { .. } => {
            Err(EncodeError::UnsupportedConstraint { variant: "Exists" })
        }
    }
}

/// Encode a [`ValueExpr`] as a Z3 [`Int`].
///
/// Float literals and Bool variables produce a [`EncodeError::SortMismatch`].
/// Use [`encode_value_real`] when the expression may involve floating-point values.
pub fn encode_value_int<'ctx>(
    expr: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Int<'ctx>, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Integer(n)) => Ok(Int::from_i64(ctx.z3, *n)),

        ValueExpr::Literal(LiteralValue::Float(_)) => Err(EncodeError::SortMismatch {
            expected: "Int",
            found: "Real",
        }),

        ValueExpr::Literal(LiteralValue::Bool(_)) => Err(EncodeError::SortMismatch {
            expected: "Int",
            found: "Bool",
        }),

        ValueExpr::Literal(LiteralValue::Nothing) => {
            Err(EncodeError::UnsupportedConstraint { variant: "Nothing" })
        }

        ValueExpr::Literal(LiteralValue::Text(_)) => Err(EncodeError::UnsupportedConstraint {
            variant: "Text-literal",
        }),

        ValueExpr::Ref(path) => {
            let dyn_var = ctx.require_var(path)?;
            dyn_var.as_int().ok_or(EncodeError::SortMismatch {
                expected: "Int",
                found: "non-Int",
            })
        }

        ValueExpr::Old(inner) => encode_old_int(inner, ctx),

        ValueExpr::Arithmetic { op, left, right } => {
            let l = encode_value_int(left, ctx)?;
            let r = encode_value_int(right, ctx)?;
            apply_arith_int(op, &l, &r)
        }

        ValueExpr::Call { .. } => Err(EncodeError::UnsupportedConstraint { variant: "Call" }),

        ValueExpr::Set(_) => Err(EncodeError::UnsupportedConstraint { variant: "Set" }),
    }
}

/// Encode a [`ValueExpr`] as a Z3 [`Real`].
///
/// Integer literals and Int-typed variables are promoted to Real automatically.
/// Bool variables and text literals produce an error.
///
/// # Float precision
/// Floating-point literals are approximated as rationals scaled by 10 000.
/// This gives 4 decimal places of precision — sufficient for AIL v0.1 percentage
/// and amount contracts.
pub fn encode_value_real<'ctx>(
    expr: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Real<'ctx>, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Integer(n)) => Ok(Int::from_i64(ctx.z3, *n).to_real()),

        ValueExpr::Literal(LiteralValue::Float(f)) => f64_to_real(ctx.z3, *f),

        ValueExpr::Literal(LiteralValue::Bool(_)) => Err(EncodeError::SortMismatch {
            expected: "Real",
            found: "Bool",
        }),

        ValueExpr::Literal(LiteralValue::Nothing) => {
            Err(EncodeError::UnsupportedConstraint { variant: "Nothing" })
        }

        ValueExpr::Literal(LiteralValue::Text(_)) => Err(EncodeError::UnsupportedConstraint {
            variant: "Text-literal",
        }),

        ValueExpr::Ref(path) => {
            let dyn_var = ctx.require_var(path)?;
            // Real vars first, then Int promoted to Real.
            if let Some(r) = dyn_var.as_real() {
                Ok(r)
            } else if let Some(i) = dyn_var.as_int() {
                Ok(i.to_real())
            } else {
                Err(EncodeError::SortMismatch {
                    expected: "Real",
                    found: "Bool",
                })
            }
        }

        ValueExpr::Old(inner) => encode_old_real(inner, ctx),

        ValueExpr::Arithmetic { op, left, right } => {
            let l = encode_value_real(left, ctx)?;
            let r = encode_value_real(right, ctx)?;
            apply_arith_real(op, &l, &r)
        }

        ValueExpr::Call { .. } => Err(EncodeError::UnsupportedConstraint { variant: "Call" }),

        ValueExpr::Set(_) => Err(EncodeError::UnsupportedConstraint { variant: "Set" }),
    }
}

/// Encode a [`ValueExpr`] as a Z3 [`Bool`].
///
/// Only `Literal(Bool(_))`, Bool-registered `Ref`, and Bool-registered `Old(Ref)` succeed.
pub fn encode_value_bool<'ctx>(
    expr: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Bool(b)) => Ok(Bool::from_bool(ctx.z3, *b)),

        ValueExpr::Ref(path) => {
            let dyn_var = ctx.require_var(path)?;
            dyn_var.as_bool().ok_or(EncodeError::SortMismatch {
                expected: "Bool",
                found: "non-Bool",
            })
        }

        ValueExpr::Old(inner) => {
            if let ValueExpr::Ref(path) = inner.as_ref() {
                let dyn_var = ctx.require_old_var(path)?;
                dyn_var.as_bool().ok_or(EncodeError::SortMismatch {
                    expected: "Bool",
                    found: "non-Bool",
                })
            } else {
                Err(EncodeError::UnsupportedConstraint {
                    variant: "Old-non-Ref",
                })
            }
        }

        _ => Err(EncodeError::SortMismatch {
            expected: "Bool",
            found: "non-Bool",
        }),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Return `true` if `expr` contains a `Literal(Float(_))` anywhere in the tree.
///
/// Used by [`encode_compare`] to choose between Int and Real encoding.
pub(super) fn expr_has_float(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Literal(LiteralValue::Float(_)) => true,
        ValueExpr::Arithmetic { left, right, .. } => expr_has_float(left) || expr_has_float(right),
        ValueExpr::Old(inner) => expr_has_float(inner),
        ValueExpr::Call { args, .. } => args.iter().any(expr_has_float),
        ValueExpr::Set(items) => items.iter().any(expr_has_float),
        _ => false,
    }
}

/// Encode a `Compare` constraint, choosing Int or Real based on float presence.
fn encode_compare<'ctx>(
    op: &CompareOp,
    left: &ValueExpr,
    right: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    if expr_has_float(left) || expr_has_float(right) {
        let l = encode_value_real(left, ctx)?;
        let r = encode_value_real(right, ctx)?;
        apply_compare_real(op, &l, &r)
    } else {
        let l = encode_value_int(left, ctx)?;
        let r = encode_value_int(right, ctx)?;
        apply_compare_int(op, &l, &r)
    }
}

/// Apply a comparison operator to two Int operands.
fn apply_compare_int<'ctx>(
    op: &CompareOp,
    l: &Int<'ctx>,
    r: &Int<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    Ok(match op {
        CompareOp::Gte => l.ge(r),
        CompareOp::Lte => l.le(r),
        CompareOp::Gt => l.gt(r),
        CompareOp::Lt => l.lt(r),
        CompareOp::Eq | CompareOp::Is => l._eq(r),
        CompareOp::Neq | CompareOp::IsNot => l._eq(r).not(),
    })
}

/// Apply a comparison operator to two Real operands.
fn apply_compare_real<'ctx>(
    op: &CompareOp,
    l: &Real<'ctx>,
    r: &Real<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    Ok(match op {
        CompareOp::Gte => l.ge(r),
        CompareOp::Lte => l.le(r),
        CompareOp::Gt => l.gt(r),
        CompareOp::Lt => l.lt(r),
        CompareOp::Eq | CompareOp::Is => l._eq(r),
        CompareOp::Neq | CompareOp::IsNot => l._eq(r).not(),
    })
}

/// Apply an arithmetic operator to two Int operands.
///
/// # Division semantics
/// Z3 uses Euclidean integer division: `(-7) / 2 = -4`.
/// Python uses floor division: `(-7) // 2 = -4`.
/// These agree for non-negative numbers. For negative dividends the results may
/// diverge — a known v0.1 limitation. TODO: encode Python floor-division semantics
/// in a future task if contracts involve negative arithmetic.
fn apply_arith_int<'ctx>(
    op: &ArithOp,
    l: &Int<'ctx>,
    r: &Int<'ctx>,
) -> Result<Int<'ctx>, EncodeError> {
    Ok(match op {
        ArithOp::Add => l + r,
        ArithOp::Sub => l - r,
        ArithOp::Mul => l * r,
        ArithOp::Div => l / r, // Euclidean — see note above
        ArithOp::Mod => l % r, // Euclidean remainder
    })
}

/// Apply an arithmetic operator to two Real operands.
///
/// `Mod` on Real is not defined in Z3 and returns [`EncodeError::UnsupportedConstraint`].
fn apply_arith_real<'ctx>(
    op: &ArithOp,
    l: &Real<'ctx>,
    r: &Real<'ctx>,
) -> Result<Real<'ctx>, EncodeError> {
    Ok(match op {
        ArithOp::Add => l + r,
        ArithOp::Sub => l - r,
        ArithOp::Mul => l * r,
        ArithOp::Div => l / r,
        ArithOp::Mod => {
            return Err(EncodeError::UnsupportedConstraint {
                variant: "Mod-on-Real",
            })
        }
    })
}

/// Encode `In { value, collection }` where `collection` must be a literal `Set`.
fn encode_in<'ctx>(
    value: &ValueExpr,
    collection: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Bool<'ctx>, EncodeError> {
    let ValueExpr::Set(items) = collection else {
        return Err(EncodeError::UnsupportedConstraint {
            variant: "In-dynamic",
        });
    };

    if items.is_empty() {
        // Empty set membership is always false.
        return Ok(Bool::from_bool(ctx.z3, false));
    }

    let use_real = expr_has_float(value) || items.iter().any(expr_has_float);

    // Encode `value` once; reuse it for each equality check against set items.
    let disjuncts: Result<Vec<Bool<'ctx>>, EncodeError> = if use_real {
        let v = encode_value_real(value, ctx)?;
        items
            .iter()
            .map(|item| Ok(v._eq(&encode_value_real(item, ctx)?)))
            .collect()
    } else {
        let v = encode_value_int(value, ctx)?;
        items
            .iter()
            .map(|item| Ok(v._eq(&encode_value_int(item, ctx)?)))
            .collect()
    };

    let disjuncts = disjuncts?;
    let refs: Vec<&Bool<'ctx>> = disjuncts.iter().collect();
    Ok(Bool::or(ctx.z3, &refs))
}

/// Encode `Old(inner)` as an integer, requiring `inner` to be a `Ref`.
fn encode_old_int<'ctx>(
    inner: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Int<'ctx>, EncodeError> {
    let ValueExpr::Ref(path) = inner else {
        return Err(EncodeError::UnsupportedConstraint {
            variant: "Old-non-Ref",
        });
    };
    let dyn_var = ctx.require_old_var(path)?;
    dyn_var.as_int().ok_or(EncodeError::SortMismatch {
        expected: "Int",
        found: "non-Int",
    })
}

/// Encode `Old(inner)` as a real, requiring `inner` to be a `Ref`.
fn encode_old_real<'ctx>(
    inner: &ValueExpr,
    ctx: &EncodeContext<'ctx>,
) -> Result<Real<'ctx>, EncodeError> {
    let ValueExpr::Ref(path) = inner else {
        return Err(EncodeError::UnsupportedConstraint {
            variant: "Old-non-Ref",
        });
    };
    let dyn_var = ctx.require_old_var(path)?;
    if let Some(r) = dyn_var.as_real() {
        Ok(r)
    } else if let Some(i) = dyn_var.as_int() {
        Ok(i.to_real())
    } else {
        Err(EncodeError::SortMismatch {
            expected: "Real",
            found: "Bool",
        })
    }
}

/// Represent an f64 as a Z3 Real via a rational approximation scaled by 10 000.
///
/// Precision: 4 decimal places. Adequate for AIL v0.1 percentage and amount literals
/// (e.g. `3.14`, `99.99`). Values beyond the i32 range before scaling will overflow;
/// those are not expected in v0.1 contracts.
fn f64_to_real<'ctx>(z3: &'ctx z3::Context, f: f64) -> Result<Real<'ctx>, EncodeError> {
    const SCALE: i32 = 10_000;
    let scaled = (f * SCALE as f64).round() as i32;
    Ok(Real::from_real(z3, scaled, SCALE))
}
