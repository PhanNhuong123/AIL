use ail_types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, ValueExpr};

use crate::types::ImportSet;

/// Render a `ConstraintExpr` AST as a Python boolean expression string.
pub(crate) fn render_constraint_python(expr: &ConstraintExpr, imports: &mut ImportSet) -> String {
    match expr {
        ConstraintExpr::Compare { op, left, right } => {
            let l = render_value_python(left, imports);
            let r = render_value_python(right, imports);
            let op_str = compare_op_python(op);
            format!("{l} {op_str} {r}")
        }
        ConstraintExpr::In { value, collection } => {
            let v = render_value_python(value, imports);
            let c = render_value_python(collection, imports);
            format!("{v} in {c}")
        }
        ConstraintExpr::Matches { value, pattern } => {
            imports.needs_re = true;
            let v = render_value_python(value, imports);
            format!("re.fullmatch(r\"{pattern}\", {v})")
        }
        ConstraintExpr::And(terms) => terms
            .iter()
            .map(|t| render_with_parens(t, Precedence::And, imports))
            .collect::<Vec<_>>()
            .join(" and "),
        ConstraintExpr::Or(terms) => terms
            .iter()
            .map(|t| render_with_parens(t, Precedence::Or, imports))
            .collect::<Vec<_>>()
            .join(" or "),
        ConstraintExpr::Not(inner) => {
            let s = render_with_parens(inner, Precedence::Not, imports);
            format!("not {s}")
        }
        ConstraintExpr::ForAll {
            variable,
            collection,
            condition,
        } => {
            let coll = render_value_python(collection, imports);
            let cond = render_constraint_python(condition, imports);
            format!("all({cond} for {variable} in {coll})")
        }
        ConstraintExpr::Exists {
            variable,
            collection,
            condition,
        } => {
            let coll = render_value_python(collection, imports);
            let cond = render_constraint_python(condition, imports);
            format!("any({cond} for {variable} in {coll})")
        }
    }
}

/// Render a `ValueExpr` AST as a Python value expression string.
pub(crate) fn render_value_python(expr: &ValueExpr, imports: &mut ImportSet) -> String {
    match expr {
        ValueExpr::Literal(lit) => render_literal_python(lit),
        ValueExpr::Ref(path) => path.join("."),
        ValueExpr::Old(inner) => {
            // Render as `_pre_<path>` where dots in the path are replaced by underscores.
            // The corresponding snapshot assignment (`_pre_x = x`) is emitted at function
            // entry by `emit_do_function` via `collect_old_refs`.
            let path = render_value_python(inner, imports);
            format!("_pre_{}", path.replace('.', "_"))
        }
        ValueExpr::Call { name, args } => {
            let arg_strs: Vec<_> = args
                .iter()
                .map(|a| render_value_python(a, imports))
                .collect();
            format!("{name}({})", arg_strs.join(", "))
        }
        ValueExpr::Arithmetic { op, left, right } => {
            let l = render_value_with_arith_parens(left, op, imports);
            let r = render_value_with_arith_parens(right, op, imports);
            let op_str = arith_op_python(op);
            format!("{l} {op_str} {r}")
        }
        ValueExpr::Set(elements) => {
            let els: Vec<_> = elements
                .iter()
                .map(|e| render_value_python(e, imports))
                .collect();
            format!("{{{}}}", els.join(", "))
        }
    }
}

fn render_literal_python(lit: &LiteralValue) -> String {
    match lit {
        LiteralValue::Integer(n) => n.to_string(),
        LiteralValue::Float(v) => {
            if v.fract() == 0.0 {
                format!("{v:.1}")
            } else {
                format!("{v}")
            }
        }
        LiteralValue::Text(s) => format!("\"{s}\""),
        LiteralValue::Bool(true) => "True".to_owned(),
        LiteralValue::Bool(false) => "False".to_owned(),
        LiteralValue::Nothing => "None".to_owned(),
    }
}

fn compare_op_python(op: &CompareOp) -> &'static str {
    match op {
        CompareOp::Gte => ">=",
        CompareOp::Lte => "<=",
        CompareOp::Gt => ">",
        CompareOp::Lt => "<",
        CompareOp::Eq | CompareOp::Is => "==",
        CompareOp::Neq | CompareOp::IsNot => "!=",
    }
}

fn arith_op_python(op: &ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "+",
        ArithOp::Sub => "-",
        ArithOp::Mul => "*",
        ArithOp::Div => "/",
        ArithOp::Mod => "%",
    }
}

// ── Precedence helpers ──────────────────────────────────────────────────────

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

fn render_with_parens(expr: &ConstraintExpr, outer: Precedence, imports: &mut ImportSet) -> String {
    let s = render_constraint_python(expr, imports);
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

fn render_value_with_arith_parens(
    expr: &ValueExpr,
    outer_op: &ArithOp,
    imports: &mut ImportSet,
) -> String {
    let s = render_value_python(expr, imports);
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

    fn render_c(s: &str) -> (String, ImportSet) {
        let expr = constraint(s);
        let mut imports = ImportSet::new();
        let result = render_constraint_python(&expr, &mut imports);
        (result, imports)
    }

    #[test]
    fn emit_render_simple_compare_gt() {
        let (r, _) = render_c("value > 0");
        assert_eq!(r, "value > 0");
    }

    #[test]
    fn emit_render_compare_gte() {
        let (r, _) = render_c("value >= 0");
        assert_eq!(r, "value >= 0");
    }

    #[test]
    fn emit_render_is_to_eq() {
        let (r, _) = render_c("status is \"active\"");
        assert_eq!(r, "status == \"active\"");
    }

    #[test]
    fn emit_render_is_not_to_neq() {
        let (r, _) = render_c("value is not nothing");
        assert_eq!(r, "value != None");
    }

    #[test]
    fn emit_render_bool_true() {
        let (r, _) = render_c("active is true");
        assert_eq!(r, "active == True");
    }

    #[test]
    fn emit_render_matches_to_re() {
        let (r, imports) = render_c("value matches /^\\d+$/");
        assert_eq!(r, "re.fullmatch(r\"^\\d+$\", value)");
        assert!(imports.needs_re);
    }

    #[test]
    fn emit_render_and() {
        let (r, _) = render_c("a > 0 and b > 0");
        assert_eq!(r, "a > 0 and b > 0");
    }

    #[test]
    fn emit_render_or() {
        let (r, _) = render_c("a > 0 or b > 0");
        assert_eq!(r, "a > 0 or b > 0");
    }

    #[test]
    fn emit_render_not() {
        let (r, _) = render_c("not a > 0");
        assert_eq!(r, "not a > 0");
    }

    #[test]
    fn emit_render_in_set() {
        let (r, _) = render_c("status in {\"active\", \"pending\"}");
        assert_eq!(r, "status in {\"active\", \"pending\"}");
    }

    #[test]
    fn emit_render_forall() {
        let (r, _) = render_c("for all x in items, x > 0");
        assert_eq!(r, "all(x > 0 for x in items)");
    }

    #[test]
    fn emit_render_exists() {
        let (r, _) = render_c("exists x in items where x > 0");
        assert_eq!(r, "any(x > 0 for x in items)");
    }

    #[test]
    fn emit_render_compound_and_or_parens() {
        let (r, _) = render_c("a > 0 and (b > 0 or c > 0)");
        assert_eq!(r, "a > 0 and (b > 0 or c > 0)");
    }

    #[test]
    fn emit_render_arithmetic() {
        let expr = ValueExpr::Arithmetic {
            op: ArithOp::Sub,
            left: Box::new(ValueExpr::Ref(vec!["balance".into()])),
            right: Box::new(ValueExpr::Ref(vec!["amount".into()])),
        };
        let mut imports = ImportSet::new();
        let r = render_value_python(&expr, &mut imports);
        assert_eq!(r, "balance - amount");
    }

    #[test]
    fn emit_render_nothing_literal() {
        let r = render_literal_python(&LiteralValue::Nothing);
        assert_eq!(r, "None");
    }

    #[test]
    fn emit_render_old_as_pre_prefix() {
        let expr = ValueExpr::Old(Box::new(ValueExpr::Ref(vec![
            "sender".into(),
            "balance".into(),
        ])));
        let mut imports = ImportSet::new();
        let r = render_value_python(&expr, &mut imports);
        assert_eq!(r, "_pre_sender_balance");
    }

    #[test]
    fn emit_render_old_simple_ref() {
        let expr = ValueExpr::Old(Box::new(ValueExpr::Ref(vec!["amount".into()])));
        let mut imports = ImportSet::new();
        let r = render_value_python(&expr, &mut imports);
        assert_eq!(r, "_pre_amount");
    }
}
