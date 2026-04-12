use std::collections::HashSet;

use ail_types::{ConstraintExpr, ValueExpr};

/// Collect the top-level variable names directly referenced in `expr`.
///
/// Names that appear only inside `old(...)` are excluded — they belong to
/// the pre-state snapshot, not the live scope being checked. Use
/// [`collect_old_refs`] for those.
///
/// For a dotted path like `sender.balance`, only the root name (`"sender"`)
/// is returned — field access on a known variable is always valid once the
/// variable itself is in scope.
///
/// Quantifier-bound variables (`item` in `for all item in items`) are excluded:
/// they are local bindings introduced by the quantifier, not scope variables.
pub fn collect_top_level_refs(expr: &ConstraintExpr) -> Vec<String> {
    let mut names = Vec::new();
    collect_refs_in_constraint(expr, false, &HashSet::new(), &mut names);
    names
}

/// Return `true` if `expr` contains any `old(...)` usage anywhere in the tree.
pub fn check_has_old(expr: &ConstraintExpr) -> bool {
    has_old_in_constraint(expr)
}

// ─── constraint walkers ────────────────────────────────────────────────────

/// Recursively collect top-level variable references.
///
/// `inside_old` suppresses collection when descending into `Old(...)`.
/// `bound_vars` accumulates quantifier-bound variable names from enclosing
/// `ForAll`/`Exists` nodes so they are not mistakenly treated as illegal refs.
fn collect_refs_in_constraint(
    expr: &ConstraintExpr,
    inside_old: bool,
    bound_vars: &HashSet<String>,
    out: &mut Vec<String>,
) {
    match expr {
        ConstraintExpr::Compare { left, right, .. } => {
            collect_refs_in_value(left, inside_old, bound_vars, out);
            collect_refs_in_value(right, inside_old, bound_vars, out);
        }
        ConstraintExpr::In { value, collection } => {
            collect_refs_in_value(value, inside_old, bound_vars, out);
            collect_refs_in_value(collection, inside_old, bound_vars, out);
        }
        ConstraintExpr::Matches { value, .. } => {
            collect_refs_in_value(value, inside_old, bound_vars, out);
        }
        ConstraintExpr::And(children) | ConstraintExpr::Or(children) => {
            for child in children {
                collect_refs_in_constraint(child, inside_old, bound_vars, out);
            }
        }
        ConstraintExpr::Not(child) => {
            collect_refs_in_constraint(child, inside_old, bound_vars, out);
        }
        ConstraintExpr::ForAll {
            variable,
            collection,
            condition,
        } => {
            // The collection expression is evaluated in the outer scope.
            collect_refs_in_value(collection, inside_old, bound_vars, out);
            // The condition introduces `variable` as a local binding — exclude
            // it from the scope check so it is not flagged as an illegal ref.
            let mut inner_bound = bound_vars.clone();
            inner_bound.insert(variable.clone());
            collect_refs_in_constraint(condition, inside_old, &inner_bound, out);
        }
        ConstraintExpr::Exists {
            variable,
            collection,
            condition,
        } => {
            collect_refs_in_value(collection, inside_old, bound_vars, out);
            let mut inner_bound = bound_vars.clone();
            inner_bound.insert(variable.clone());
            collect_refs_in_constraint(condition, inside_old, &inner_bound, out);
        }
    }
}

fn collect_refs_in_value(
    expr: &ValueExpr,
    inside_old: bool,
    bound_vars: &HashSet<String>,
    out: &mut Vec<String>,
) {
    match expr {
        ValueExpr::Ref(parts) => {
            if !inside_old {
                if let Some(root) = parts.first() {
                    if !bound_vars.contains(root.as_str()) {
                        out.push(root.clone());
                    }
                }
            }
        }
        ValueExpr::Old(inner) => {
            // Switch into old-context: refs inside old() are NOT top-level refs.
            // They will be captured by collect_old_refs separately.
            collect_refs_in_value(inner, true, bound_vars, out);
        }
        ValueExpr::Arithmetic { left, right, .. } => {
            collect_refs_in_value(left, inside_old, bound_vars, out);
            collect_refs_in_value(right, inside_old, bound_vars, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_refs_in_value(arg, inside_old, bound_vars, out);
            }
        }
        ValueExpr::Set(elements) => {
            for elem in elements {
                collect_refs_in_value(elem, inside_old, bound_vars, out);
            }
        }
        ValueExpr::Literal(_) => {}
    }
}

// ─── old() presence check ──────────────────────────────────────────────────

fn has_old_in_constraint(expr: &ConstraintExpr) -> bool {
    match expr {
        ConstraintExpr::Compare { left, right, .. } => {
            has_old_in_value(left) || has_old_in_value(right)
        }
        ConstraintExpr::In { value, collection } => {
            has_old_in_value(value) || has_old_in_value(collection)
        }
        ConstraintExpr::Matches { value, .. } => has_old_in_value(value),
        ConstraintExpr::And(children) | ConstraintExpr::Or(children) => {
            children.iter().any(has_old_in_constraint)
        }
        ConstraintExpr::Not(child) => has_old_in_constraint(child),
        ConstraintExpr::ForAll {
            collection,
            condition,
            ..
        }
        | ConstraintExpr::Exists {
            collection,
            condition,
            ..
        } => has_old_in_value(collection) || has_old_in_constraint(condition),
    }
}

fn has_old_in_value(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Old(_) => true,
        ValueExpr::Arithmetic { left, right, .. } => {
            has_old_in_value(left) || has_old_in_value(right)
        }
        ValueExpr::Call { args, .. } => args.iter().any(has_old_in_value),
        ValueExpr::Set(elements) => elements.iter().any(has_old_in_value),
        ValueExpr::Ref(_) | ValueExpr::Literal(_) => false,
    }
}
