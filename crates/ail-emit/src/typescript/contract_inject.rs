use ail_graph::{Contract, ContractKind, NodeId};
use ail_types::{parse_constraint_expr, ConstraintExpr, ValueExpr};

use crate::errors::EmitError;
use crate::types::ContractMode;
use crate::typescript::constraint::{render_constraint_ts, render_value_ts};
use crate::typescript::import_tracker::ImportTracker;

// ── Old-ref collection ────────────────────────────────────────────────────────

fn collect_old_refs_in_value(
    expr: &ValueExpr,
    out: &mut Vec<(String, String)>,
    seen: &mut std::collections::HashSet<String>,
) {
    match expr {
        ValueExpr::Old(inner) => {
            // snapshot var = "_old_" + source_path.replace('.', "_")
            // This matches render_value_ts which formats Old as `_old_{path_with_underscores}`.
            let source = render_value_ts(inner);
            let name = format!("_old_{}", source.replace('.', "_"));
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

/// Collect unique `old()` snapshot pairs from `After` contracts only.
///
/// Returns `(snapshot_name, source_expr)` pairs, e.g.
/// `("_old_sender_balance", "sender.balance")`.
///
/// # v2.0 Limitation
/// `old(sender.field)` is safe for primitives. `old(sender)` (whole object)
/// captures a reference — mutations to `sender` will also affect the snapshot.
/// Only use `old()` on primitive fields in v2.0.
///
/// # Errors
/// Returns `EmitError::OldRefInNonAfterContract` if `old()` appears in a
/// `Before` or `Always` contract, which is always a user error.
pub(crate) fn collect_old_refs_ts(
    node_id: NodeId,
    contracts: &[Contract],
) -> Result<Vec<(String, String)>, EmitError> {
    // Reject old() in non-After contracts first.
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

fn kind_prefix(kind: &ContractKind) -> &'static str {
    match kind {
        ContractKind::Before => "PRE",
        ContractKind::After => "POST",
        ContractKind::Always => "KEEP",
    }
}

fn kind_fn(kind: &ContractKind) -> &'static str {
    match kind {
        ContractKind::Before => "pre",
        ContractKind::After => "post",
        ContractKind::Always => "keep",
    }
}

/// Render a single contract as one TypeScript line at `indent`.
///
/// - `On`: `pre(expr, "raw")` / `post(...)` / `keep(...)`; sets `tracker.needs_runtime`.
/// - `Comments`: `// PRE: raw` / `// POST: raw` / `// KEEP: raw`.
/// - `Off` / `Test`: empty vec.
pub(crate) fn render_contract_lines_ts(
    node_id: NodeId,
    contract: &Contract,
    indent: &str,
    mode: &ContractMode,
    tracker: &mut ImportTracker,
) -> Result<Vec<String>, EmitError> {
    match mode {
        ContractMode::Off | ContractMode::Test => return Ok(vec![]),
        _ => {}
    }

    let raw = &contract.expression.0;
    let parsed = parse_constraint_expr(raw).map_err(|e| EmitError::TsConstraintParseError {
        node_id,
        expression: raw.clone(),
        message: e.to_string(),
    })?;

    let line = match mode {
        ContractMode::On => {
            let ts_expr = render_constraint_ts(&parsed);
            let fn_name = kind_fn(&contract.kind);
            tracker.needs_runtime = true;
            format!("{indent}{fn_name}({ts_expr}, \"{raw}\");")
        }
        ContractMode::Comments => {
            let prefix = kind_prefix(&contract.kind);
            format!("{indent}// {prefix}: {raw}")
        }
        ContractMode::Off | ContractMode::Test => unreachable!("checked above"),
    };

    Ok(vec![line])
}

/// Render `Before` + `Always` contracts (pre-body checks).
pub(crate) fn render_before_contract_lines_ts(
    node_id: NodeId,
    contracts: &[Contract],
    indent: &str,
    mode: &ContractMode,
    tracker: &mut ImportTracker,
) -> Result<Vec<String>, EmitError> {
    let mut lines = Vec::new();
    for c in contracts {
        if c.kind == ContractKind::After {
            continue;
        }
        lines.extend(render_contract_lines_ts(node_id, c, indent, mode, tracker)?);
    }
    Ok(lines)
}

/// Render `After` + `Always` contracts (post-body checks, injected before return).
///
/// Always contracts are re-checked here in addition to pre-body so that
/// invariants hold both entering and leaving the function.
pub(crate) fn render_after_contract_lines_ts(
    node_id: NodeId,
    contracts: &[Contract],
    indent: &str,
    mode: &ContractMode,
    tracker: &mut ImportTracker,
) -> Result<Vec<String>, EmitError> {
    let mut lines = Vec::new();
    // After contracts (postconditions) first.
    for c in contracts {
        if c.kind != ContractKind::After {
            continue;
        }
        lines.extend(render_contract_lines_ts(node_id, c, indent, mode, tracker)?);
    }
    // Always contracts re-checked after body.
    for c in contracts {
        if c.kind != ContractKind::Always {
            continue;
        }
        lines.extend(render_contract_lines_ts(node_id, c, indent, mode, tracker)?);
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

    // ── collect_old_refs_ts ───────────────────────────────────────────────────

    #[test]
    fn collect_old_refs_ts_single() {
        let contracts = vec![make_contract(
            ContractKind::After,
            "result.balance is old(sender.balance) - amount",
        )];
        let refs = collect_old_refs_ts(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_old_sender_balance");
        assert_eq!(refs[0].1, "sender.balance");
    }

    #[test]
    fn collect_old_refs_ts_deduplicates() {
        let contracts = vec![
            make_contract(ContractKind::After, "x is old(y) - 1"),
            make_contract(ContractKind::After, "x > old(y)"),
        ];
        let refs = collect_old_refs_ts(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_old_y");
    }

    #[test]
    fn collect_old_refs_ts_ignores_before_contracts() {
        let contracts = vec![
            make_contract(ContractKind::Before, "x > 0"),
            make_contract(ContractKind::After, "y is old(z)"),
        ];
        let refs = collect_old_refs_ts(dummy_id(), &contracts).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, "_old_z");
    }

    #[test]
    fn collect_old_refs_ts_errors_on_old_in_before_contract() {
        let contracts = vec![make_contract(ContractKind::Before, "x is old(y)")];
        let err = collect_old_refs_ts(dummy_id(), &contracts).unwrap_err();
        assert!(matches!(err, EmitError::OldRefInNonAfterContract { .. }));
    }

    #[test]
    fn collect_old_refs_ts_errors_on_old_in_always_contract() {
        let contracts = vec![make_contract(ContractKind::Always, "x is old(y)")];
        let err = collect_old_refs_ts(dummy_id(), &contracts).unwrap_err();
        assert!(matches!(err, EmitError::OldRefInNonAfterContract { .. }));
    }

    // ── render_contract_lines_ts ──────────────────────────────────────────────

    #[test]
    fn render_mode_on_before_emits_pre() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut tracker = ImportTracker::new();
        let lines =
            render_contract_lines_ts(dummy_id(), &contract, "  ", &ContractMode::On, &mut tracker)
                .unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("pre("));
        assert!(tracker.needs_runtime);
    }

    #[test]
    fn render_mode_on_after_emits_post() {
        let contract = make_contract(ContractKind::After, "result > 0");
        let mut tracker = ImportTracker::new();
        let lines =
            render_contract_lines_ts(dummy_id(), &contract, "  ", &ContractMode::On, &mut tracker)
                .unwrap();
        assert!(lines[0].contains("post("));
    }

    #[test]
    fn render_mode_on_always_emits_keep() {
        let contract = make_contract(ContractKind::Always, "balance >= 0");
        let mut tracker = ImportTracker::new();
        let lines =
            render_contract_lines_ts(dummy_id(), &contract, "  ", &ContractMode::On, &mut tracker)
                .unwrap();
        assert!(lines[0].contains("keep("));
    }

    #[test]
    fn render_mode_comments_before_emits_pre_comment() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut tracker = ImportTracker::new();
        let lines = render_contract_lines_ts(
            dummy_id(),
            &contract,
            "  ",
            &ContractMode::Comments,
            &mut tracker,
        )
        .unwrap();
        assert_eq!(lines[0], "  // PRE: x > 0");
        assert!(!tracker.needs_runtime);
    }

    #[test]
    fn render_mode_comments_after_emits_post_comment() {
        let contract = make_contract(ContractKind::After, "result > 0");
        let mut tracker = ImportTracker::new();
        let lines = render_contract_lines_ts(
            dummy_id(),
            &contract,
            "  ",
            &ContractMode::Comments,
            &mut tracker,
        )
        .unwrap();
        assert_eq!(lines[0], "  // POST: result > 0");
    }

    #[test]
    fn render_mode_comments_always_emits_keep_comment() {
        let contract = make_contract(ContractKind::Always, "balance >= 0");
        let mut tracker = ImportTracker::new();
        let lines = render_contract_lines_ts(
            dummy_id(),
            &contract,
            "  ",
            &ContractMode::Comments,
            &mut tracker,
        )
        .unwrap();
        assert_eq!(lines[0], "  // KEEP: balance >= 0");
    }

    #[test]
    fn render_mode_off_returns_empty() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut tracker = ImportTracker::new();
        let lines = render_contract_lines_ts(
            dummy_id(),
            &contract,
            "  ",
            &ContractMode::Off,
            &mut tracker,
        )
        .unwrap();
        assert!(lines.is_empty());
        assert!(!tracker.needs_runtime);
    }

    #[test]
    fn render_mode_test_returns_empty() {
        let contract = make_contract(ContractKind::Before, "x > 0");
        let mut tracker = ImportTracker::new();
        let lines = render_contract_lines_ts(
            dummy_id(),
            &contract,
            "  ",
            &ContractMode::Test,
            &mut tracker,
        )
        .unwrap();
        assert!(lines.is_empty());
    }

    #[test]
    fn render_after_contract_includes_always_recheck() {
        let contracts = vec![
            make_contract(ContractKind::After, "result > 0"),
            make_contract(ContractKind::Always, "balance >= 0"),
        ];
        let mut tracker = ImportTracker::new();
        let lines = render_after_contract_lines_ts(
            dummy_id(),
            &contracts,
            "  ",
            &ContractMode::On,
            &mut tracker,
        )
        .unwrap();
        // post(result > 0, ...) then keep(balance >= 0, ...)
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("post("));
        assert!(lines[1].contains("keep("));
    }
}
