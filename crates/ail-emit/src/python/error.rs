use ail_graph::Node;

use crate::constants::PYTHON_INDENT;
use crate::errors::EmitError;
use crate::python::type_map::resolve_python_type;
use crate::types::ImportSet;

/// Emit a Python `Exception` subclass for an Error node.
///
/// `error E carries f1:T1, f2:T2` becomes a class that extends `Exception`
/// with typed attributes and a formatted error message.
pub(crate) fn emit_error_node(node: &Node, imports: &mut ImportSet) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::ErrorNodeMissingName { node_id: node.id })?;

    // Build docstring from metadata.
    let docstring = if node.metadata.carries.is_empty() {
        format!("error {name}")
    } else {
        let carries_doc: Vec<_> = node
            .metadata
            .carries
            .iter()
            .map(|f| format!("{}:{}", f.name, f.type_ref))
            .collect();
        format!("error {name} carries {}", carries_doc.join(", "))
    };

    let i = PYTHON_INDENT;
    let mut lines = Vec::new();

    lines.push(format!("class {name}(Exception):"));
    lines.push(format!("{i}\"\"\"{docstring}\"\"\""));

    if node.metadata.carries.is_empty() {
        // No carries — empty exception body.
        lines.push(format!("{i}pass"));
    } else {
        lines.push(String::new());

        // __init__ with carried fields as parameters.
        let params: Vec<_> = node
            .metadata
            .carries
            .iter()
            .map(|f| {
                let py_type = resolve_python_type(&f.type_ref, imports);
                format!("{}: {py_type}", f.name)
            })
            .collect();

        lines.push(format!(
            "{i}def __init__(self, {}) -> None:",
            params.join(", ")
        ));

        // Assign carried fields.
        for field in &node.metadata.carries {
            lines.push(format!("{i}{i}self.{n} = {n}", n = field.name));
        }

        // super().__init__ with formatted message.
        let fmt_parts: Vec<_> = node
            .metadata
            .carries
            .iter()
            .map(|f| format!("{}={{{n}!r}}", f.name, n = f.name))
            .collect();
        lines.push(format!(
            "{i}{i}super().__init__(f\"{name}({})\")",
            fmt_parts.join(", ")
        ));
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

    fn make_error_node(name: &str, carries: Vec<(&str, &str)>) -> Node {
        let mut node = Node {
            id: NodeId::new(),
            intent: format!("{name} error"),
            pattern: Pattern::Error,
            children: None,
            expression: None,
            contracts: Vec::new(),
            metadata: NodeMetadata::default(),
        };
        node.metadata.name = Some(name.to_owned());
        node.metadata.carries = carries
            .into_iter()
            .map(|(n, t)| Field {
                name: n.to_owned(),
                type_ref: t.to_owned(),
            })
            .collect();
        node
    }

    #[test]
    fn emit_error_insufficient_balance() {
        let node = make_error_node(
            "InsufficientBalanceError",
            vec![
                ("current_balance", "WalletBalance"),
                ("requested_amount", "PositiveAmount"),
            ],
        );
        let mut imports = ImportSet::new();
        let result = emit_error_node(&node, &mut imports).unwrap();

        assert!(result.contains("class InsufficientBalanceError(Exception):"));
        assert!(result.contains("def __init__(self, current_balance: WalletBalance, requested_amount: PositiveAmount) -> None:"));
        assert!(result.contains("self.current_balance = current_balance"));
        assert!(result.contains("self.requested_amount = requested_amount"));
        assert!(result.contains("super().__init__"));
    }

    #[test]
    fn emit_error_with_no_carries() {
        let node = make_error_node("GenericError", vec![]);
        let mut imports = ImportSet::new();
        let result = emit_error_node(&node, &mut imports).unwrap();

        assert!(result.contains("class GenericError(Exception):"));
        assert!(result.contains("pass"));
        assert!(!result.contains("def __init__"));
    }

    #[test]
    fn emit_error_missing_name_returns_error() {
        let mut node = make_error_node("X", vec![]);
        node.metadata.name = None;
        let mut imports = ImportSet::new();
        let err = emit_error_node(&node, &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::ErrorNodeMissingName { .. }));
    }

    #[test]
    fn emit_error_docstring_includes_carries() {
        let node = make_error_node("E", vec![("code", "integer"), ("msg", "text")]);
        let mut imports = ImportSet::new();
        let result = emit_error_node(&node, &mut imports).unwrap();

        assert!(result.contains("\"\"\"error E carries code:integer, msg:text\"\"\""));
    }
}
