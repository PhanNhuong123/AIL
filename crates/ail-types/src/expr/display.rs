use std::fmt;

use crate::types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};

// ── LiteralValue ─────────────────────────────────────────────────────────────

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LiteralValue::Integer(n) => write!(f, "{n}"),
            LiteralValue::Float(v) => {
                // Always include a decimal point so roundtrip re-parses as Float.
                if v.fract() == 0.0 {
                    write!(f, "{v:.1}")
                } else {
                    write!(f, "{v}")
                }
            }
            LiteralValue::Text(s) => write!(f, "\"{s}\""),
            LiteralValue::Bool(b) => write!(f, "{b}"),
            LiteralValue::Nothing => write!(f, "nothing"),
        }
    }
}

// ── CompareOp ────────────────────────────────────────────────────────────────

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CompareOp::Gte => ">=",
            CompareOp::Lte => "<=",
            CompareOp::Gt => ">",
            CompareOp::Lt => "<",
            CompareOp::Eq => "==",
            CompareOp::Neq => "!=",
            CompareOp::Is => "is",
            CompareOp::IsNot => "is not",
        };
        write!(f, "{s}")
    }
}

// ── ArithOp ──────────────────────────────────────────────────────────────────

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
            ArithOp::Mod => "%",
        };
        write!(f, "{s}")
    }
}

// ── ValueExpr ────────────────────────────────────────────────────────────────

impl fmt::Display for ValueExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueExpr::Literal(lit) => write!(f, "{lit}"),
            ValueExpr::Ref(path) => write!(f, "{}", path.join(".")),
            ValueExpr::Old(inner) => write!(f, "old({inner})"),
            ValueExpr::Call { name, args } => {
                write!(f, "{name}(")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            ValueExpr::Arithmetic { op, left, right } => {
                write_value_with_parens(f, left, op)?;
                write!(f, " {op} ")?;
                write_value_with_parens(f, right, op)
            }
            ValueExpr::Set(elements) => {
                write!(f, "{{")?;
                for (i, el) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{el}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// Write a value expression, adding parentheses when it is a lower-precedence
/// arithmetic expression appearing as an operand inside a higher-precedence one.
fn write_value_with_parens(
    f: &mut fmt::Formatter<'_>,
    expr: &ValueExpr,
    outer_op: &ArithOp,
) -> fmt::Result {
    if let ValueExpr::Arithmetic { op: inner_op, .. } = expr {
        if arith_precedence(inner_op) < arith_precedence(outer_op) {
            return write!(f, "({expr})");
        }
    }
    write!(f, "{expr}")
}

fn arith_precedence(op: &ArithOp) -> u8 {
    match op {
        ArithOp::Add | ArithOp::Sub => 1,
        ArithOp::Mul | ArithOp::Div | ArithOp::Mod => 2,
    }
}

// ── ConstraintExpr ───────────────────────────────────────────────────────────

impl fmt::Display for ConstraintExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintExpr::Compare { op, left, right } => {
                write!(f, "{left} {op} {right}")
            }
            ConstraintExpr::In { value, collection } => {
                write!(f, "{value} in {collection}")
            }
            ConstraintExpr::Matches { value, pattern } => {
                write!(f, "{value} matches /{pattern}/")
            }
            ConstraintExpr::And(terms) => {
                for (i, term) in terms.iter().enumerate() {
                    if i > 0 {
                        write!(f, " and ")?;
                    }
                    write_constraint_with_parens(f, term, ConstraintPrecedence::And)?;
                }
                Ok(())
            }
            ConstraintExpr::Or(terms) => {
                for (i, term) in terms.iter().enumerate() {
                    if i > 0 {
                        write!(f, " or ")?;
                    }
                    write_constraint_with_parens(f, term, ConstraintPrecedence::Or)?;
                }
                Ok(())
            }
            ConstraintExpr::Not(inner) => {
                write!(f, "not ")?;
                write_constraint_with_parens(f, inner, ConstraintPrecedence::Not)
            }
            ConstraintExpr::ForAll { variable, collection, condition } => {
                write!(f, "for all {variable} in {collection}, {condition}")
            }
            ConstraintExpr::Exists { variable, collection, condition } => {
                write!(f, "exists {variable} in {collection} where {condition}")
            }
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum ConstraintPrecedence {
    Or = 1,
    And = 2,
    Not = 3,
    Compare = 4,
}

fn constraint_precedence(expr: &ConstraintExpr) -> ConstraintPrecedence {
    match expr {
        ConstraintExpr::Or(_) => ConstraintPrecedence::Or,
        ConstraintExpr::And(_) => ConstraintPrecedence::And,
        ConstraintExpr::Not(_) => ConstraintPrecedence::Not,
        _ => ConstraintPrecedence::Compare,
    }
}

fn write_constraint_with_parens(
    f: &mut fmt::Formatter<'_>,
    expr: &ConstraintExpr,
    outer: ConstraintPrecedence,
) -> fmt::Result {
    if constraint_precedence(expr) < outer {
        write!(f, "({expr})")
    } else {
        write!(f, "{expr}")
    }
}
