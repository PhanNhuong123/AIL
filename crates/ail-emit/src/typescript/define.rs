use ail_graph::{ContractKind, Node};
use ail_types::parse_constraint_expr;

use crate::errors::EmitError;
use crate::typescript::constraint::render_constraint_ts;
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::type_map::{collect_user_types, resolve_ts_type, to_snake_case};

const I: &str = "  ";

/// Emit a TypeScript branded type + validator + predicate for a Define node.
///
/// `define T:base where constraint` becomes:
/// - `export type T = <base_ts> & { readonly __brand: 'T' };`
/// - `export function createT(value: <base_ts>): T { ... }`
/// - `export function isT(value: <base_ts>): value is T { ... }`
pub(crate) fn emit_define_node(
    node: &Node,
    tracker: &mut ImportTracker,
) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsDefineNodeMissingName { node_id: node.id })?;

    let base_type = node
        .metadata
        .base_type
        .as_deref()
        .ok_or(EmitError::TsDefineNodeMissingBaseType { node_id: node.id })?;

    let ts_base = resolve_ts_type(base_type);
    let is_integer = base_type.trim() == "integer";

    // Collect constraint sources for this define node. Parser stores the
    // `where <expr>` clause in `node.expression` (see
    // crates/ail-text/src/parser/walker.rs), not in `node.contracts`, so the
    // Always contract list may be empty even when a constraint is present.
    // Prefer contracts when available; fall back to `node.expression`.
    let mut constraint_sources: Vec<String> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Always)
        .map(|c| c.expression.0.clone())
        .collect();
    if constraint_sources.is_empty() {
        if let Some(expr) = node.expression.as_ref() {
            let trimmed = expr.0.trim();
            if !trimmed.is_empty() {
                constraint_sources.push(trimmed.to_owned());
            }
        }
    }

    // Register user-defined types referenced in the base type.
    for user_type in collect_user_types(base_type) {
        let snake = to_snake_case(&user_type);
        tracker.register(&user_type, &format!("./{snake}"), TypeKind::Define);
    }

    // Build constraint check lines.
    let mut checks: Vec<(String, String)> = Vec::new(); // (ts_expr, display_str)

    if is_integer {
        checks.push(("Number.isInteger(value)".to_owned(), "integer".to_owned()));
    }

    for raw in &constraint_sources {
        let parsed = parse_constraint_expr(raw).map_err(|e| EmitError::TsConstraintParseError {
            node_id: node.id,
            expression: raw.clone(),
            message: e.to_string(),
        })?;
        let ts_expr = render_constraint_ts(&parsed);
        checks.push((ts_expr, raw.clone()));
    }

    // Docstring from metadata.
    let docstring = if constraint_sources.is_empty() {
        format!("{name} — define {name}:{base_type}")
    } else {
        format!(
            "{name} — define {name}:{base_type} where {}",
            constraint_sources.join(" and ")
        )
    };

    let mut lines = Vec::new();

    // Branded type alias.
    lines.push(format!("/** {docstring} */"));
    lines.push(format!(
        "export type {name} = {ts_base} & {{ readonly __brand: '{name}' }};"
    ));
    lines.push(String::new());

    // Factory function.
    lines.push(format!(
        "export function create{name}(value: {ts_base}): {name} {{"
    ));
    for (ts_expr, display) in &checks {
        lines.push(format!("{I}if (!({ts_expr})) {{"));
        lines.push(format!(
            "{I}{I}throw new Error(`{name} constraint violated: {display} (got ${{value}})`);",
        ));
        lines.push(format!("{I}}}"));
    }
    lines.push(format!("{I}return value as {name};"));
    lines.push("}".to_owned());
    lines.push(String::new());

    // Predicate function.
    lines.push(format!(
        "export function is{name}(value: {ts_base}): value is {name} {{"
    ));
    if checks.is_empty() {
        lines.push(format!("{I}return true;"));
    } else {
        let cond = checks
            .iter()
            .map(|(e, _)| e.as_str())
            .collect::<Vec<_>>()
            .join(" && ");
        lines.push(format!("{I}return {cond};"));
    }
    lines.push("}".to_owned());

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

    fn make_define(name: &str, base: &str, constraint: Option<&str>) -> Node {
        let mut node = Node {
            id: NodeId::new(),
            intent: format!("{name} type"),
            pattern: Pattern::Define,
            children: None,
            expression: None,
            contracts: Vec::new(),
            metadata: NodeMetadata::default(),
        };
        node.metadata.name = Some(name.to_owned());
        node.metadata.base_type = Some(base.to_owned());
        if let Some(c) = constraint {
            node.contracts.push(Contract {
                kind: ContractKind::Always,
                expression: Expression(c.to_owned()),
            });
        }
        node
    }

    #[test]
    fn ts_define_emits_branded_type() {
        let node = make_define("WalletBalance", "number", None);
        let mut tracker = ImportTracker::new();
        let result = emit_define_node(&node, &mut tracker).unwrap();
        assert!(result.contains(
            "export type WalletBalance = number & { readonly __brand: 'WalletBalance' };"
        ));
    }

    #[test]
    fn ts_define_emits_create_function() {
        let node = make_define("WalletBalance", "number", Some("value >= 0"));
        let mut tracker = ImportTracker::new();
        let result = emit_define_node(&node, &mut tracker).unwrap();
        assert!(
            result.contains("export function createWalletBalance(value: number): WalletBalance {")
        );
        assert!(result.contains("if (!(value >= 0))"));
    }

    #[test]
    fn ts_define_emits_is_predicate() {
        let node = make_define("WalletBalance", "number", Some("value >= 0"));
        let mut tracker = ImportTracker::new();
        let result = emit_define_node(&node, &mut tracker).unwrap();
        assert!(result
            .contains("export function isWalletBalance(value: number): value is WalletBalance {"));
    }

    #[test]
    fn ts_define_integer_adds_isinteger_check() {
        let node = make_define("Count", "integer", None);
        let mut tracker = ImportTracker::new();
        let result = emit_define_node(&node, &mut tracker).unwrap();
        assert!(result.contains("Number.isInteger(value)"));
    }

    #[test]
    fn ts_define_missing_name_errors() {
        let mut node = make_define("X", "number", None);
        node.metadata.name = None;
        let mut tracker = ImportTracker::new();
        let err = emit_define_node(&node, &mut tracker).unwrap_err();
        assert!(matches!(err, EmitError::TsDefineNodeMissingName { .. }));
    }
}
