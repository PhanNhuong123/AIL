use ail_types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};
use z3::ast::{Bool, Int, Real};

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
pub fn encode_constraint(expr: &ConstraintExpr, ctx: &EncodeContext) -> Result<Bool, EncodeError> {
    match expr {
        ConstraintExpr::Compare { op, left, right } => encode_compare(op, left, right, ctx),

        ConstraintExpr::And(children) => {
            let bools: Vec<Bool> = children
                .iter()
                .map(|c| encode_constraint(c, ctx))
                .collect::<Result<_, _>>()?;
            Ok(Bool::and(&bools))
        }

        ConstraintExpr::Or(children) => {
            let bools: Vec<Bool> = children
                .iter()
                .map(|c| encode_constraint(c, ctx))
                .collect::<Result<_, _>>()?;
            Ok(Bool::or(&bools))
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
pub fn encode_value_int(expr: &ValueExpr, ctx: &EncodeContext) -> Result<Int, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Integer(n)) => Ok(Int::from_i64(*n)),

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
pub fn encode_value_real(expr: &ValueExpr, ctx: &EncodeContext) -> Result<Real, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Integer(n)) => Ok(Int::from_i64(*n).to_real()),

        ValueExpr::Literal(LiteralValue::Float(f)) => f64_to_real(*f),

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
pub fn encode_value_bool(expr: &ValueExpr, ctx: &EncodeContext) -> Result<Bool, EncodeError> {
    match expr {
        ValueExpr::Literal(LiteralValue::Bool(b)) => Ok(Bool::from_bool(*b)),

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

/// Return `true` if any `Ref` or `Old(Ref)` in `expr` is registered as a
/// Real-sort variable in `ctx`.
///
/// Used by [`encode_compare`] to promote Int-literal comparisons to Real when
/// at least one operand variable is declared as a `number`-based type.
fn expr_references_real_var(expr: &ValueExpr, ctx: &EncodeContext) -> bool {
    match expr {
        ValueExpr::Ref(path) => ctx.get_var(path).is_some_and(|v| v.as_real().is_some()),
        ValueExpr::Old(inner) => {
            if let ValueExpr::Ref(path) = inner.as_ref() {
                ctx.get_old_var(path).is_some_and(|v| v.as_real().is_some())
            } else {
                expr_references_real_var(inner, ctx)
            }
        }
        ValueExpr::Arithmetic { left, right, .. } => {
            expr_references_real_var(left, ctx) || expr_references_real_var(right, ctx)
        }
        _ => false,
    }
}

/// Return `true` if `expr` contains a `Literal(Bool(_))` at the top level.
///
/// Bool literals never appear inside arithmetic or sets, so a shallow check is
/// sufficient.
fn expr_has_bool_literal(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::Literal(LiteralValue::Bool(_)))
}

/// Return `true` if `expr` is a `Ref` or `Old(Ref)` registered as a Bool-sort
/// variable in `ctx`.
///
/// Used by [`encode_compare`] to route Bool equality checks (e.g.
/// `result == true`) to [`encode_value_bool`] instead of the Int path.
fn expr_references_bool_var(expr: &ValueExpr, ctx: &EncodeContext) -> bool {
    match expr {
        ValueExpr::Ref(path) => ctx.get_var(path).is_some_and(|v| v.as_bool().is_some()),
        ValueExpr::Old(inner) => {
            if let ValueExpr::Ref(path) = inner.as_ref() {
                ctx.get_old_var(path).is_some_and(|v| v.as_bool().is_some())
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Encode a `Compare` constraint, choosing Bool, Int, or Real based on operand sorts.
///
/// Bool is selected when either operand is a Bool literal or references a
/// Bool-registered variable (e.g. a `boolean`-typed parameter or `result`).
/// Only equality operators (`Eq`, `Is`, `Neq`, `IsNot`) are valid for Bool.
///
/// Otherwise Real is selected when either operand contains a float literal OR
/// references a variable registered with Real sort (e.g. a `number`-typed
/// parameter); integer literals are promoted to Real in that case. All remaining
/// cases encode as Int.
fn encode_compare(
    op: &CompareOp,
    left: &ValueExpr,
    right: &ValueExpr,
    ctx: &EncodeContext,
) -> Result<Bool, EncodeError> {
    let use_bool = expr_has_bool_literal(left)
        || expr_has_bool_literal(right)
        || expr_references_bool_var(left, ctx)
        || expr_references_bool_var(right, ctx);

    if use_bool {
        let l = encode_value_bool(left, ctx)?;
        let r = encode_value_bool(right, ctx)?;
        return apply_compare_bool(op, &l, &r);
    }

    let use_real = expr_has_float(left)
        || expr_has_float(right)
        || expr_references_real_var(left, ctx)
        || expr_references_real_var(right, ctx);

    if use_real {
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
fn apply_compare_int(op: &CompareOp, l: &Int, r: &Int) -> Result<Bool, EncodeError> {
    Ok(match op {
        CompareOp::Gte => l.ge(r),
        CompareOp::Lte => l.le(r),
        CompareOp::Gt => l.gt(r),
        CompareOp::Lt => l.lt(r),
        CompareOp::Eq | CompareOp::Is => l.eq(r),
        CompareOp::Neq | CompareOp::IsNot => l.eq(r).not(),
    })
}

/// Apply a comparison operator to two Bool operands.
///
/// Only equality operators are defined on Bool; ordering operators produce
/// [`EncodeError::UnsupportedConstraint`].
fn apply_compare_bool(op: &CompareOp, l: &Bool, r: &Bool) -> Result<Bool, EncodeError> {
    match op {
        CompareOp::Eq | CompareOp::Is => Ok(l.eq(r)),
        CompareOp::Neq | CompareOp::IsNot => Ok(l.eq(r).not()),
        CompareOp::Gt | CompareOp::Gte | CompareOp::Lt | CompareOp::Lte => {
            Err(EncodeError::UnsupportedConstraint {
                variant: "Bool-ordering",
            })
        }
    }
}

/// Apply a comparison operator to two Real operands.
fn apply_compare_real(op: &CompareOp, l: &Real, r: &Real) -> Result<Bool, EncodeError> {
    Ok(match op {
        CompareOp::Gte => l.ge(r),
        CompareOp::Lte => l.le(r),
        CompareOp::Gt => l.gt(r),
        CompareOp::Lt => l.lt(r),
        CompareOp::Eq | CompareOp::Is => l.eq(r),
        CompareOp::Neq | CompareOp::IsNot => l.eq(r).not(),
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
fn apply_arith_int(op: &ArithOp, l: &Int, r: &Int) -> Result<Int, EncodeError> {
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
fn apply_arith_real(op: &ArithOp, l: &Real, r: &Real) -> Result<Real, EncodeError> {
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
fn encode_in(
    value: &ValueExpr,
    collection: &ValueExpr,
    ctx: &EncodeContext,
) -> Result<Bool, EncodeError> {
    let ValueExpr::Set(items) = collection else {
        return Err(EncodeError::UnsupportedConstraint {
            variant: "In-dynamic",
        });
    };

    if items.is_empty() {
        // Empty set membership is always false.
        return Ok(Bool::from_bool(false));
    }

    let use_real = expr_has_float(value) || items.iter().any(expr_has_float);

    // Encode `value` once; reuse it for each equality check against set items.
    let disjuncts: Vec<Bool> = if use_real {
        let v = encode_value_real(value, ctx)?;
        items
            .iter()
            .map(|item| encode_value_real(item, ctx).map(|i| v.eq(&i)))
            .collect::<Result<_, _>>()?
    } else {
        let v = encode_value_int(value, ctx)?;
        items
            .iter()
            .map(|item| encode_value_int(item, ctx).map(|i| v.eq(&i)))
            .collect::<Result<_, _>>()?
    };

    Ok(Bool::or(&disjuncts))
}

/// Encode `Old(inner)` as an integer, requiring `inner` to be a `Ref`.
fn encode_old_int(inner: &ValueExpr, ctx: &EncodeContext) -> Result<Int, EncodeError> {
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
fn encode_old_real(inner: &ValueExpr, ctx: &EncodeContext) -> Result<Real, EncodeError> {
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
/// Precision: 4 decimal places. Adequate for AIL v0.1 percentage and amount
/// literals (e.g. `3.14`, `99.99`).
///
/// `Real::from_rational` declares an `i64` signature in z3 0.20, but the
/// underlying `Z3_mk_real` C FFI takes `c_int` (32 bits) and casts silently.
/// To prevent a wrong-rational corruption at runtime, guard against
/// `|scaled| > i32::MAX` (i.e. `|f| > 214_748.3647`) and surface an
/// `UnsupportedConstraint` instead. Values within range are unaffected.
fn f64_to_real(f: f64) -> Result<Real, EncodeError> {
    const SCALE: i64 = 10_000;
    let scaled = (f * SCALE as f64).round() as i64;
    if scaled > i32::MAX as i64 || scaled < i32::MIN as i64 {
        return Err(EncodeError::UnsupportedConstraint {
            variant: "float-out-of-i32-range",
        });
    }
    Ok(Real::from_rational(scaled, SCALE))
}
