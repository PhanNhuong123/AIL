use ail_graph::{Contract, ContractKind, NodeId};
use ail_types::{parse_constraint_expr, ConstraintExpr, ValueExpr};

use crate::errors::EmitError;
use crate::python::constraint::{render_constraint_python, render_value_python};
use crate::types::{ContractMode, ImportSet};

// ── Old-ref collection ────────────────────────────────────────────────────────

/// Walk a `ValueExpr` and append `(snapshot_name, source_path)` pairs for each
/// `ValueExpr::Old(...)` found.  Only looks at the given expression; the caller
/// is responsible for driving collection over the constraint tree.
fn collect_old_refs_in_value(
    expr: &ValueExpr,
    out: &mut Vec<(String, String)>,
    seen: &mut std::collections::HashSet<String>,
) {
    match expr {
        ValueExpr::Old(inner) => {
            // Assumption: the expression inside old() is always a simple path ref
            // (e.g. `old(sender.balance)`), which requires no imports to render.
            // A throwaway ImportSet is used deliberately to keep this helper
            // import-free; call-expression args inside old() are not supported.
            let source = render_value_python(inner, &mut ImportSet::new());
            let name = format!("_pre_{}", source.replace('.', "_"));
            if seen.insert(name.clone()) {
                out.push((name, source));
            }
        }
        ValueExpr::Arithmetic { left, right, .. } => {
            collect_old_refs_in_value(left, out, seen);
            collect_old_refs_in_value(right, out, seen);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_old_refs_in_value(arg, out, seen);
            }
        }
        ValueExpr::Set(elems) => {
            for e in elems {
                collect_old_refs_in_value(e, out, seen);
            }
        }
        ValueExpr::Literal(_) | ValueExpr::Ref(_) => {}
    }
}

fn collect_old_refs_in_constraint(
    expr: &ConstraintExpr,
    out: &mut Vec<(String, String)>,
    seen: &mut std::collections::HashSet<String>,
) {
    match expr {
        ConstraintExpr::Compare { left, right, .. } => {
            collect_old_refs_in_value(left, out, seen);
            collect_old_refs_in_value(right, out, seen);
        }
        ConstraintExpr::In { value, collection } => {
            collect_old_refs_in_value(value, out, seen);
            collect_old_refs_in_value(collection, out, seen);
        }
        ConstraintExpr::Matches { value, .. } => {
            collect_old_refs_in_value(value, out, seen);
        }
        ConstraintExpr::And(terms) | ConstraintExpr::Or(terms) => {
            for t in terms {
                collect_old_refs_in_constraint(t, out, seen);
            }
        }
        ConstraintExpr::Not(inner) => {
            collect_old_refs_in_constraint(inner, out, seen);
        }
        ConstraintExpr::ForAll { condition, .. } | ConstraintExpr::Exists { condition, .. } => {
            collect_old_refs_in_constraint(condition, out, seen);
        }
    }
}

/// Detect whether a `ConstraintExpr` contains any `ValueExpr::Old(...)`.
fn constraint_has_old(expr: &ConstraintExpr) -> bool {
    match expr {
        ConstraintExpr::Compare { left, right, .. } => value_has_old(left) || value_has_old(right),
        ConstraintExpr::In { value, collection } => {
            value_has_old(value) || value_has_old(collection)
        }
        ConstraintExpr::Matches { value, .. } => value_has_old(value),
        ConstraintExpr::And(terms) | ConstraintExpr::Or(terms) => {
            terms.iter().any(constraint_has_old)
        }
        ConstraintExpr::Not(inner) => constraint_has_old(inner),
        ConstraintExpr::ForAll { condition, .. } | ConstraintExpr::Exists { condition, .. } => {
            constraint_has_old(condition)
        }
    }
}

fn value_has_old(expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::Old(_) => true,
        ValueExpr::Arithmetic { left, right, .. } => value_has_old(left) || value_has_old(right),
        ValueExpr::Call { args, .. } => args.iter().any(value_has_old),
        ValueExpr::Set(elems) => elems.iter().any(value_has_old),
        ValueExpr::Literal(_) | ValueExpr::Ref(_) => false,
    }
}

/// Collect unique `old()` snapshot pairs from **After** contracts only.
///
/// Returns `(snapshot_name, source_expr)` — e.g. `("_pre_sender_balance", "sender.balance")`.
///
/// # Errors
/// Returns `EmitError::OldRefInNonAfterContract` if an `old()` reference is
/// found in a `Before` or `Always` contract (which is always a user error).
pub(crate) fn collect_old_refs(
    node_id: NodeId,
    contracts: &[Contract],
) -> Result<Vec<(String, String)>, EmitError> {
    // Check for old() in non-After contracts first.
    for c in contracts {
        if c.kind == ContractKind::After {
            continue;
        }
        if let Ok(parsed) = parse_constraint_expr(&c.expression.0) {
            if constraint_has_old(&parsed) {
                return Err(EmitError::OldRefInNonAfterContract { node_id });
            }
        }
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for c in contracts {
        if c.kind != ContractKind::After {
            continue;
        }
        if let Ok(parsed) = parse_constraint_expr(&c.expression.0) {
            collect_old_refs_in_constraint(&parsed, &mut out, &mut seen);
        }
    }

    Ok(out)
}

// ── Contract line rendering ───────────────────────────────────────────────────

fn kind_label(kind: &ContractKind) -> &'static str {
    match kind {
        ContractKind::Before => "before",
        ContractKind::After => "after",
        ContractKind::Always => "always",
    }
}

/// Render a single contract as one or more Python assertion lines at `indent`.
///
/// Returns an empty vec when `mode` is `Off`.
pub(crate) fn render_contract_lines(
    node_id: NodeId,
    contract: &Contract,
    indent: &str,
    mode: &ContractMode,
    imports: &mut ImportSet,
) -> Result<Vec<String>, EmitError> {
    if *mode == ContractMode::Off {
        return Ok(vec![]);
    }

    let raw = &contract.expression.0;
    let parsed = parse_constraint_expr(raw).map_err(|e| EmitError::ConstraintParseError {
        node_id,
        expression: raw.clone(),
        message: e.to_string(),
    })?;

    let py_expr = render_constraint_python(&parsed, imports);
    let label = kind_label(&contract.kind);
    let line = match mode {
        ContractMode::On => {
            format!("{indent}assert {py_expr}  # {label}: {raw}")
        }
        ContractMode::Comments => {
            format!("{indent}# assert {py_expr}  # {label}: {raw}")
        }
        ContractMode::Off => unreachable!("checked above"),
    };

    Ok(vec![line])
}

/// Render before-contract assertion lines (Before + Always contracts).
///
/// Returns `EmitError::OldRefInNonAfterContract` if any of those contracts
/// reference `old()`.
pub(crate) fn render_before_contract_lines(
    node_id: NodeId,
    contracts: &[Contract],
    indent: &str,
    mode: &ContractMode,
    imports: &mut ImportSet,
) -> Result<Vec<String>, EmitError> {
    let mut lines = Vec::new();
    for c in contracts {
        if c.kind == ContractKind::After {
            continue;
        }
        let mut contract_lines = render_contract_lines(node_id, c, indent, mode, imports)?;
        lines.append(&mut contract_lines);
    }
    Ok(lines)
}

/// Render after-contract assertion lines (After contracts only).
pub(crate) fn render_after_contract_lines(
    node_id: NodeId,
    contracts: &[Contract],
    indent: &str,
    mode: &ContractMode,
    imports: &mut ImportSet,
) -> Result<Vec<String>, EmitError> {
    let mut lines = Vec::new();
    for c in contracts {
        if c.kind != ContractKind::After {
            continue;
        }
        let mut contract_lines = render_contract_lines(node_id, c, indent, mode, imports)?;
        lines.append(&mut contract_lines);
    }
    Ok(lines)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::{Contract, ContractKind, Expression, NodeId};

    fn make_contract(kind: ContractKind, expr: &str) -> Contract {
        Contract {
            kind,
            expression: Expression(expr.to_owned()),
        }
    }

    fn dummy_id() -> NodeId {
        NodeId::default()
    }

    // ── collect_old_refs ──────────────────────────────────────────────────────

    #[test]
    fn collect_old_refs_single() {
        let contracts = vec![make_contract(
            ContractKind::After,
            "result.balance is old(sender.balance) - amount",
        )];
        let refs = collect_old_refs(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_pre_sender_balance");
        assert_eq!(refs[0].1, "sender.balance");
    }

    #[test]
    fn collect_old_refs_deduplicates() {
        let contracts = vec![
            make_contract(ContractKind::After, "x is old(y) - 1"),
            make_contract(ContractKind::After, "x > old(y)"),
        ];
        let refs = collect_old_refs(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_pre_y");
    }

    #[test]
    fn collect_old_refs_nested_in_arithmetic() {
        let contracts = vec![make_contract(ContractKind::After, "x is old(a) - old(b)")];
        let refs = collect_old_refs(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn collect_old_refs_ignores_before_contracts() {
        let contracts = vec![
            make_contract(ContractKind::Before, "x > 0"),
            make_contract(ContractKind::After, "y is old(z)"),
        ];
        let refs = collect_old_refs(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_pre_z");
    }

    #[test]
    fn collect_old_refs_errors_on_old_in_before_contract() {
        let contracts = vec![make_contract(ContractKind::Before, "x is old(y)")];
        let err = collect_old_refs(dummy_id(), &contracts).unwrap_err();
        assert!(matches!(err, EmitError::OldRefInNonAfterContract { .. }));
    }

    #[test]
    fn collect_old_refs_errors_on_old_in_always_contract() {
        let contracts = vec![make_contract(ContractKind::Always, "x is old(y)")];
        let err = collect_old_refs(dummy_id(), &contracts).unwrap_err();
        assert!(matches!(err, EmitError::OldRefInNonAfterContract { .. }));
    }

    // ── render_contract_lines ─────────────────────────────────────────────────

    #[test]
    fn contract_mode_on_renders_assert() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut imports = ImportSet::new();
        let lines = render_contract_lines(
            dummy_id(),
            &contract,
            "    ",
            &ContractMode::On,
            &mut imports,
        )
        .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "    assert x > 0  # before: x > 0");
    }

    #[test]
    fn contract_mode_comments_renders_hash_assert() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut imports = ImportSet::new();
        let lines = render_contract_lines(
            dummy_id(),
            &contract,
            "    ",
            &ContractMode::Comments,
            &mut imports,
        )
        .unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "    # assert x > 0  # before: x > 0");
    }

    #[test]
    fn contract_mode_off_returns_empty() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut imports = ImportSet::new();
        let lines = render_contract_lines(
            dummy_id(),
            &contract,
            "    ",
            &ContractMode::Off,
            &mut imports,
        )
        .unwrap();
        assert!(lines.is_empty());
    }

    #[test]
    fn render_after_contract_line_has_after_label() {
        let contract = make_contract(ContractKind::After, "result > 0");
        let mut imports = ImportSet::new();
        let lines = render_contract_lines(
            dummy_id(),
            &contract,
            "    ",
            &ContractMode::On,
            &mut imports,
        )
        .unwrap();
        assert!(lines[0].contains("# after: result > 0"));
    }
}
