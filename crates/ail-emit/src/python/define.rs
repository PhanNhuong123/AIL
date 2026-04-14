use ail_graph::{ContractKind, Node};
use ail_types::parse_constraint_expr;

use crate::constants::PYTHON_INDENT;
use crate::errors::EmitError;
use crate::python::constraint::render_constraint_python;
use crate::python::type_map::resolve_python_type;
use crate::types::ImportSet;

/// Emit a Python class for a Define node.
///
/// `define T:base where constraint` becomes a Python class with `__init__`
/// that validates the constraint using `keep()`, plus `__repr__` and `__eq__`.
pub(crate) fn emit_define_node(node: &Node, imports: &mut ImportSet) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::DefineNodeMissingName { node_id: node.id })?;

    let base_type = node
        .metadata
        .base_type
        .as_deref()
        .ok_or(EmitError::DefineNodeMissingBaseType { node_id: node.id })?;

    let python_type = resolve_python_type(base_type, imports);

    // Collect Always contracts (define constraints).
    let always_contracts: Vec<_> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Always)
        .collect();

    // Build the docstring from metadata.
    let docstring = if always_contracts.is_empty() {
        format!("define {name}:{base_type}")
    } else {
        let constraint_text: Vec<_> = always_contracts
            .iter()
            .map(|c| c.expression.0.clone())
            .collect();
        format!(
            "define {name}:{base_type} where {}",
            constraint_text.join(" and ")
        )
    };

    // Parse and render constraints to Python.
    let mut keep_lines = Vec::new();
    for contract in &always_contracts {
        let raw = &contract.expression.0;
        let parsed = parse_constraint_expr(raw).map_err(|e| EmitError::ConstraintParseError {
            node_id: node.id,
            expression: raw.clone(),
            message: e.to_string(),
        })?;

        // Split top-level And into separate keep() calls for better error messages.
        let constraint_parts = flatten_top_level_and(&parsed);
        imports.needs_keep = true;

        for part in constraint_parts {
            let py_expr = render_constraint_python(part, imports);
            let display = part.to_string();
            keep_lines.push(format!(
                "{I}{I}keep({py_expr}, \"{name}: {display}\")",
                I = PYTHON_INDENT
            ));
        }
    }

    let i = PYTHON_INDENT;
    let mut lines = Vec::new();

    // Class definition.
    lines.push(format!("class {name}:"));
    lines.push(format!("{i}\"\"\"{docstring}\"\"\""));
    lines.push(String::new());

    // __init__
    lines.push(format!(
        "{i}def __init__(self, value: {python_type}) -> None:"
    ));
    for keep_line in &keep_lines {
        lines.push(keep_line.clone());
    }
    lines.push(format!("{i}{i}self.value = value"));

    lines.push(String::new());

    // __repr__
    lines.push(format!("{i}def __repr__(self) -> str:"));
    lines.push(format!("{i}{i}return f\"{name}(value={{self.value!r}})\""));
    lines.push(String::new());

    // __eq__
    lines.push(format!("{i}def __eq__(self, other: object) -> bool:"));
    lines.push(format!(
        "{i}{i}return isinstance(other, {name}) and self.value == other.value"
    ));

    Ok(lines.join("\n"))
}

/// Flatten top-level `And(terms)` into individual constraints.
/// Non-And constraints are returned as a single-element slice.
fn flatten_top_level_and(expr: &ail_types::ConstraintExpr) -> Vec<&ail_types::ConstraintExpr> {
    match expr {
        ail_types::ConstraintExpr::And(terms) => terms.iter().collect(),
        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

    fn make_define_node(name: &str, base: &str, constraint: Option<&str>) -> Node {
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
    fn emit_define_positive_amount() {
        let node = make_define_node("PositiveAmount", "number", Some("value > 0"));
        let mut imports = ImportSet::new();
        let result = emit_define_node(&node, &mut imports).unwrap();

        assert!(result.contains("class PositiveAmount:"));
        assert!(result.contains("def __init__(self, value: float) -> None:"));
        assert!(result.contains("keep(value > 0, \"PositiveAmount: value > 0\")"));
        assert!(result.contains("self.value = value"));
        assert!(result.contains("def __repr__(self) -> str:"));
        assert!(result.contains("def __eq__(self, other: object) -> bool:"));
        assert!(imports.needs_keep);
    }

    #[test]
    fn emit_define_compound_constraint_splits_keep() {
        let node = make_define_node("Percentage", "number", Some("value >= 0 and value <= 100"));
        let mut imports = ImportSet::new();
        let result = emit_define_node(&node, &mut imports).unwrap();

        // Should have two separate keep() calls.
        assert!(result.contains("keep(value >= 0, \"Percentage: value >= 0\")"));
        assert!(result.contains("keep(value <= 100, \"Percentage: value <= 100\")"));
    }

    #[test]
    fn emit_define_with_regex_constraint() {
        let node = make_define_node("UserId", "text", Some("value matches /usr_[a-z0-9]+/"));
        let mut imports = ImportSet::new();
        let result = emit_define_node(&node, &mut imports).unwrap();

        assert!(result.contains("re.fullmatch"));
        assert!(imports.needs_re);
    }

    #[test]
    fn emit_define_no_constraint() {
        let node = make_define_node("RawAmount", "number", None);
        let mut imports = ImportSet::new();
        let result = emit_define_node(&node, &mut imports).unwrap();

        assert!(result.contains("class RawAmount:"));
        assert!(result.contains("def __init__(self, value: float) -> None:"));
        assert!(result.contains("self.value = value"));
        assert!(!result.contains("keep("));
        assert!(!imports.needs_keep);
    }

    #[test]
    fn emit_define_missing_name_returns_error() {
        let mut node = make_define_node("X", "number", None);
        node.metadata.name = None;
        let mut imports = ImportSet::new();
        let err = emit_define_node(&node, &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::DefineNodeMissingName { .. }));
    }

    #[test]
    fn emit_define_missing_base_type_returns_error() {
        let mut node = make_define_node("X", "number", None);
        node.metadata.base_type = None;
        let mut imports = ImportSet::new();
        let err = emit_define_node(&node, &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::DefineNodeMissingBaseType { .. }));
    }
}
