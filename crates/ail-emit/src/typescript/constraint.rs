use ail_types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};

/// Render a `ConstraintExpr` AST as a TypeScript boolean expression string.
///
/// Differences from Python rendering:
/// - `==` / `is` → `===`
/// - `!=` / `is not` → `!==`
/// - `True/False` → `true/false`
/// - `None/nothing` → `null`
/// - regex → `.match(...)` pattern (simplified; v2.0 keeps it readable)
pub(crate) fn render_constraint_ts(expr: &ConstraintExpr) -> String {
    match expr {
        ConstraintExpr::Compare { op, left, right } => {
            let l = render_value_ts(left);
            let r = render_value_ts(right);
            let op_str = compare_op_ts(op);
            format!("{l} {op_str} {r}")
        }
        ConstraintExpr::In { value, collection } => {
            let v = render_value_ts(value);
            let c = render_value_ts(collection);
            // TypeScript: Array.isArray check or .includes(); use includes for set literals.
            format!("{c}.includes({v})")
        }
        ConstraintExpr::Matches { value, pattern } => {
            let v = render_value_ts(value);
            format!("/^(?:{pattern})$/.test({v})")
        }
        ConstraintExpr::And(terms) => terms
            .iter()
            .map(|t| render_with_parens(t, Precedence::And))
            .collect::<Vec<_>>()
            .join(" && "),
        ConstraintExpr::Or(terms) => terms
            .iter()
            .map(|t| render_with_parens(t, Precedence::Or))
            .collect::<Vec<_>>()
            .join(" || "),
        ConstraintExpr::Not(inner) => {
            let s = render_constraint_ts(inner);
            format!("!({s})")
        }
        ConstraintExpr::ForAll {
            variable,
            collection,
            condition,
        } => {
            let coll = render_value_ts(collection);
            let cond = render_constraint_ts(condition);
            format!("{coll}.every(({variable}) => {cond})")
        }
        ConstraintExpr::Exists {
            variable,
            collection,
            condition,
        } => {
            let coll = render_value_ts(collection);
            let cond = render_constraint_ts(condition);
            format!("{coll}.some(({variable}) => {cond})")
        }
    }
}

/// Render a `ValueExpr` AST as a TypeScript value expression string.
pub(crate) fn render_value_ts(expr: &ValueExpr) -> String {
    match expr {
        ValueExpr::Literal(lit) => render_literal_ts(lit),
        ValueExpr::Ref(path) => path.join("."),
        ValueExpr::Old(inner) => {
            let path = render_value_ts(inner);
            format!("_old_{}", path.replace('.', "_"))
        }
        ValueExpr::Call { name, args } => {
            let arg_strs: Vec<_> = args.iter().map(render_value_ts).collect();
            format!("{name}({})", arg_strs.join(", "))
        }
        ValueExpr::Arithmetic { op, left, right } => {
            let l = render_value_with_arith_parens(left, op);
            let r = render_value_with_arith_parens(right, op);
            let op_str = arith_op_ts(op);
            format!("{l} {op_str} {r}")
        }
        ValueExpr::Set(elements) => {
            let els: Vec<_> = elements.iter().map(render_value_ts).collect();
            format!("[{}]", els.join(", "))
        }
    }
}

fn render_literal_ts(lit: &LiteralValue) -> String {
    match lit {
        LiteralValue::Integer(n) => n.to_string(),
        LiteralValue::Float(v) => {
            if v.fract() == 0.0 {
                format!("{v:.1}")
            } else {
                format!("{v}")
            }
        }
        LiteralValue::Text(s) => format!("'{s}'"),
        LiteralValue::Bool(true) => "true".to_owned(),
        LiteralValue::Bool(false) => "false".to_owned(),
        LiteralValue::Nothing => "null".to_owned(),
    }
}

fn compare_op_ts(op: &CompareOp) -> &'static str {
    match op {
        CompareOp::Gte => ">=",
        CompareOp::Lte => "<=",
        CompareOp::Gt => ">",
        CompareOp::Lt => "<",
        CompareOp::Eq | CompareOp::Is => "===",
        CompareOp::Neq | CompareOp::IsNot => "!==",
    }
}

fn arith_op_ts(op: &ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
        ArithOp::Mod => "%",
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Precedence {
    Or = 1,
    And = 2,
    Not = 3,
    Atom = 4,
}

fn precedence_of(expr: &ConstraintExpr) -> Precedence {
    match expr {
        ConstraintExpr::Or(_) => Precedence::Or,
        ConstraintExpr::And(_) => Precedence::And,
        ConstraintExpr::Not(_) => Precedence::Not,
        _ => Precedence::Atom,
    }
}

fn render_with_parens(expr: &ConstraintExpr, outer: Precedence) -> String {
    let s = render_constraint_ts(expr);
    if precedence_of(expr) < outer {
        format!("({s})")
    } else {
        s
    }
}

fn arith_precedence(op: &ArithOp) -> u8 {
    match op {
        ArithOp::Add | ArithOp::Sub => 1,
        ArithOp::Mul | ArithOp::Div | ArithOp::Mod => 2,
    }
}

fn render_value_with_arith_parens(expr: &ValueExpr, outer_op: &ArithOp) -> String {
    let s = render_value_ts(expr);
    if let ValueExpr::Arithmetic { op: inner_op, .. } = expr {
        if arith_precedence(inner_op) < arith_precedence(outer_op) {
            return format!("({s})");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constraint(s: &str) -> ConstraintExpr {
        ail_types::parse_constraint_expr(s).expect("parse failed")
    }

    fn render(s: &str) -> String {
        render_constraint_ts(&constraint(s))
    }

    #[test]
    fn ts_render_simple_gte() {
        assert_eq!(render("value >= 0"), "value >= 0");
    }

    #[test]
    fn ts_render_is_becomes_strict_eq() {
        assert_eq!(render("status is \"active\""), "status === 'active'");
    }

    #[test]
    fn ts_render_is_not_becomes_strict_neq() {
        assert_eq!(render("value is not nothing"), "value !== null");
    }

    #[test]
    fn ts_render_bool_true_lowercase() {
        assert_eq!(render("active is true"), "active === true");
    }

    #[test]
    fn ts_render_bool_false_lowercase() {
        assert_eq!(render("active is false"), "active === false");
    }

    #[test]
    fn ts_render_nothing_to_null() {
        let expr = ConstraintExpr::Compare {
            op: CompareOp::Is,
            left: Box::new(ValueExpr::Ref(vec!["x".into()])),
            right: Box::new(ValueExpr::Literal(LiteralValue::Nothing)),
        };
        assert_eq!(render_constraint_ts(&expr), "x === null");
    }

    #[test]
    fn ts_render_and_uses_double_ampersand() {
        assert_eq!(render("a > 0 and b > 0"), "a > 0 && b > 0");
    }

    #[test]
    fn ts_render_or_uses_double_pipe() {
        assert_eq!(render("a > 0 or b > 0"), "a > 0 || b > 0");
    }

    #[test]
    fn ts_render_not_uses_bang() {
        // TypeScript `!` binds tighter than `>`, so the argument must be parenthesised.
        assert_eq!(render("not a > 0"), "!(a > 0)");
    }

    #[test]
    fn ts_render_matches_to_regex_test() {
        let result = render("value matches /^\\d+$/");
        assert!(result.contains(".test(value)"));
    }

    #[test]
    fn ts_render_old_uses_old_prefix() {
        let expr = ValueExpr::Old(Box::new(ValueExpr::Ref(vec![
            "sender".into(),
            "balance".into(),
        ])));
        assert_eq!(render_value_ts(&expr), "_old_sender_balance");
    }
}
