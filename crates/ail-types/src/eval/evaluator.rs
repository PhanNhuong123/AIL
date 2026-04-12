use regex::Regex;

use crate::errors::EvalError;
use crate::eval::{EvalContext, Value};
use crate::types::{ArithOp, CompareOp, ConstraintExpr, ValueExpr};

// ── Public API ────────────────────────────────────────────────────────────────

/// Evaluate a boolean constraint expression against the given context.
///
/// Both functions are pure — they do not mutate the context.
///
/// # Regex performance note
/// `Matches` patterns are compiled on every call. For hot paths (e.g. CIC verify
/// loops checking the same invariant after each action), callers should cache
/// evaluation results rather than re-evaluating identical expressions.
pub fn eval_constraint(expr: &ConstraintExpr, ctx: &EvalContext) -> Result<bool, EvalError> {
    match expr {
        ConstraintExpr::Compare { op, left, right } => {
            let left_val = eval_value(left, ctx)?;
            let right_val = eval_value(right, ctx)?;
            apply_compare(*op, &left_val, &right_val)
        }

        ConstraintExpr::In { value, collection } => {
            let val = eval_value(value, ctx)?;
            let coll = eval_value(collection, ctx)?;
            match coll {
                Value::List(items) => Ok(items.contains(&val)),
                other => Err(EvalError::TypeMismatch {
                    expected: "list".into(),
                    actual: other.kind_name().into(),
                    value_preview: other.to_string(),
                }),
            }
        }

        ConstraintExpr::Matches { value, pattern } => {
            let val = eval_value(value, ctx)?;
            match val {
                Value::Text(s) => {
                    let re = Regex::new(pattern).map_err(|e| {
                        EvalError::InvalidRegex(pattern.clone(), e.to_string())
                    })?;
                    Ok(re.is_match(&s))
                }
                other => Err(EvalError::TypeMismatch {
                    expected: "text".into(),
                    actual: other.kind_name().into(),
                    value_preview: other.to_string(),
                }),
            }
        }

        ConstraintExpr::And(exprs) => {
            for e in exprs {
                if !eval_constraint(e, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true) // empty And is vacuously true
        }

        ConstraintExpr::Or(exprs) => {
            for e in exprs {
                if eval_constraint(e, ctx)? {
                    return Ok(true);
                }
            }
            Ok(false) // empty Or is vacuously false
        }

        ConstraintExpr::Not(inner) => Ok(!eval_constraint(inner, ctx)?),

        ConstraintExpr::ForAll { variable, collection, condition } => {
            let coll = eval_value(collection, ctx)?;
            let items = require_list(coll)?;
            for item in items {
                let child_ctx = ctx.bind(variable, item);
                if !eval_constraint(condition, &child_ctx)? {
                    return Ok(false);
                }
            }
            Ok(true) // vacuously true for empty collection
        }

        ConstraintExpr::Exists { variable, collection, condition } => {
            let coll = eval_value(collection, ctx)?;
            let items = require_list(coll)?;
            for item in items {
                let child_ctx = ctx.bind(variable, item);
                if eval_constraint(condition, &child_ctx)? {
                    return Ok(true);
                }
            }
            Ok(false) // vacuously false for empty collection
        }
    }
}

/// Evaluate a value expression against the given context.
pub fn eval_value(expr: &ValueExpr, ctx: &EvalContext) -> Result<Value, EvalError> {
    match expr {
        ValueExpr::Literal(lit) => Ok(Value::from(lit)),

        ValueExpr::Ref(path) => {
            // path is guaranteed non-empty by the parser
            let first = path.first().expect("Ref path must be non-empty");
            let root = ctx
                .bindings
                .get(first.as_str())
                .ok_or_else(|| EvalError::UndefinedVariable(first.clone()))?;

            let mut current = root;
            for field in path.iter().skip(1) {
                match current {
                    Value::Record(map) => {
                        current = map.get(field.as_str()).ok_or_else(|| {
                            EvalError::UndefinedField {
                                field: field.clone(),
                                value_kind: "record".into(),
                            }
                        })?;
                    }
                    other => {
                        return Err(EvalError::UndefinedField {
                            field: field.clone(),
                            value_kind: other.kind_name().into(),
                        });
                    }
                }
            }
            Ok(current.clone())
        }

        ValueExpr::Old(inner) => {
            let old_ctx = ctx.as_old_context()?;
            eval_value(inner, &old_ctx)
        }

        ValueExpr::Call { name, args } => match name.as_str() {
            "len" => {
                if args.len() != 1 {
                    return Err(EvalError::WrongArgCount {
                        name: "len".into(),
                        expected: 1,
                        actual: args.len(),
                    });
                }
                let arg = eval_value(&args[0], ctx)?;
                match arg {
                    Value::List(items) => Ok(Value::Integer(items.len() as i64)),
                    Value::Text(s) => Ok(Value::Integer(s.chars().count() as i64)),
                    other => Err(EvalError::TypeMismatch {
                        expected: "list or text".into(),
                        actual: other.kind_name().into(),
                        value_preview: other.to_string(),
                    }),
                }
            }
            other => Err(EvalError::UnknownFunction(other.into())),
        },

        ValueExpr::Arithmetic { op, left, right } => {
            let left_val = eval_value(left, ctx)?;
            let right_val = eval_value(right, ctx)?;
            apply_arithmetic(*op, left_val, right_val)
        }

        ValueExpr::Set(items) => {
            let values: Result<Vec<Value>, EvalError> =
                items.iter().map(|item| eval_value(item, ctx)).collect();
            Ok(Value::List(values?))
        }
    }
}

// ── Private helpers ────────────────────────────────────────────────────────────

fn require_list(val: Value) -> Result<Vec<Value>, EvalError> {
    match val {
        Value::List(items) => Ok(items),
        other => Err(EvalError::TypeMismatch {
            expected: "list".into(),
            actual: other.kind_name().into(),
            value_preview: other.to_string(),
        }),
    }
}

fn apply_arithmetic(op: ArithOp, left: Value, right: Value) -> Result<Value, EvalError> {
    // Promote Integer×Float → Float; Integer×Integer → Integer.
    match (&left, &right) {
        (Value::Integer(a), Value::Integer(b)) => apply_arith_int(op, *a, *b),
        (Value::Float(a), Value::Float(b)) => apply_arith_float(op, *a, *b),
        (Value::Integer(a), Value::Float(b)) => apply_arith_float(op, *a as f64, *b),
        (Value::Float(a), Value::Integer(b)) => apply_arith_float(op, *a, *b as f64),
        _ => {
            // Report whichever operand is not numeric.
            let bad = if !matches!(left, Value::Integer(_) | Value::Float(_)) {
                &left
            } else {
                &right
            };
            Err(EvalError::TypeMismatch {
                expected: "number".into(),
                actual: bad.kind_name().into(),
                value_preview: bad.to_string(),
            })
        }
    }
}

fn ordering_to_int(o: std::cmp::Ordering) -> i32 {
    match o {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn apply_arith_int(op: ArithOp, a: i64, b: i64) -> Result<Value, EvalError> {
    match op {
        ArithOp::Add => Ok(Value::Integer(a + b)),
        ArithOp::Sub => Ok(Value::Integer(a - b)),
        ArithOp::Mul => Ok(Value::Integer(a * b)),
        ArithOp::Div => {
            if b == 0 {
                Err(EvalError::DivisionByZero)
            } else {
                Ok(Value::Integer(a / b))
            }
        }
        ArithOp::Mod => {
            if b == 0 {
                Err(EvalError::DivisionByZero)
            } else {
                Ok(Value::Integer(a % b))
            }
        }
    }
}

fn apply_arith_float(op: ArithOp, a: f64, b: f64) -> Result<Value, EvalError> {
    match op {
        ArithOp::Add => Ok(Value::Float(a + b)),
        ArithOp::Sub => Ok(Value::Float(a - b)),
        ArithOp::Mul => Ok(Value::Float(a * b)),
        ArithOp::Div => {
            if b == 0.0 {
                Err(EvalError::DivisionByZero)
            } else {
                Ok(Value::Float(a / b))
            }
        }
        ArithOp::Mod => {
            if b == 0.0 {
                Err(EvalError::DivisionByZero)
            } else {
                Ok(Value::Float(a % b))
            }
        }
    }
}

fn apply_compare(op: CompareOp, left: &Value, right: &Value) -> Result<bool, EvalError> {
    // Nothing is only comparable with Is/Eq and IsNot/Neq; ordering → TypeMismatch.
    if matches!(left, Value::Nothing) || matches!(right, Value::Nothing) {
        return match op {
            CompareOp::Is | CompareOp::Eq => Ok(left == right),
            CompareOp::IsNot | CompareOp::Neq => Ok(left != right),
            _ => Err(EvalError::TypeMismatch {
                expected: "comparable value".into(),
                actual: "nothing".into(),
                value_preview: "nothing".into(),
            }),
        };
    }

    match op {
        CompareOp::Eq | CompareOp::Is => compare_eq(left, right),
        CompareOp::Neq | CompareOp::IsNot => compare_eq(left, right).map(|r| !r),
        CompareOp::Gte => compare_order(left, right).map(|o| o >= 0),
        CompareOp::Lte => compare_order(left, right).map(|o| o <= 0),
        CompareOp::Gt => compare_order(left, right).map(|o| o > 0),
        CompareOp::Lt => compare_order(left, right).map(|o| o < 0),
    }
}

/// Equality comparison — cross-type returns false (no coercion for equality).
fn compare_eq(left: &Value, right: &Value) -> Result<bool, EvalError> {
    // Integer vs Float coercion for equality
    match (left, right) {
        (Value::Integer(a), Value::Float(b)) => Ok((*a as f64) == *b),
        (Value::Float(a), Value::Integer(b)) => Ok(*a == (*b as f64)),
        _ => Ok(left == right),
    }
}

/// Ordering comparison — returns -1, 0, or 1.
/// Coerces Integer↔Float. Errors on non-numeric or incompatible types.
fn compare_order(left: &Value, right: &Value) -> Result<i32, EvalError> {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => Ok(ordering_to_int(a.cmp(b))),
        (Value::Float(a), Value::Float(b)) => {
            a.partial_cmp(b).map(ordering_to_int).ok_or(EvalError::TypeMismatch {
                expected: "non-NaN number".into(),
                actual: "NaN".into(),
                value_preview: format!("{a} or {b}"),
            })
        }
        (Value::Integer(a), Value::Float(b)) => {
            (*a as f64).partial_cmp(b).map(ordering_to_int).ok_or(EvalError::TypeMismatch {
                expected: "non-NaN number".into(),
                actual: "NaN".into(),
                value_preview: b.to_string(),
            })
        }
        (Value::Float(a), Value::Integer(b)) => {
            a.partial_cmp(&(*b as f64)).map(ordering_to_int).ok_or(EvalError::TypeMismatch {
                expected: "non-NaN number".into(),
                actual: "NaN".into(),
                value_preview: a.to_string(),
            })
        }
        (Value::Text(a), Value::Text(b)) => Ok(ordering_to_int(a.cmp(b))),
        _ => Err(EvalError::TypeMismatch {
            expected: "number or text".into(),
            actual: format!("{} and {}", left.kind_name(), right.kind_name()),
            value_preview: format!("{left} and {right}"),
        }),
    }
}
