use ail_graph::Node;

use crate::constants::PYTHON_INDENT;
use crate::errors::EmitError;
use crate::python::type_map::resolve_python_type;
use crate::types::ImportSet;

/// Emit a Python `@dataclass(frozen=True)` for a Describe node.
///
/// `describe T as f1:T1, f2:T2` becomes a frozen dataclass with typed fields.
/// Field order is preserved from `metadata.fields`.
pub(crate) fn emit_describe_node(
    node: &Node,
    imports: &mut ImportSet,
) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::DescribeNodeMissingName { node_id: node.id })?;

    imports.needs_dataclass = true;

    // Build docstring from metadata.
    let fields_doc: Vec<_> = node
        .metadata
        .fields
        .iter()
        .map(|f| format!("{}:{}", f.name, f.type_ref))
        .collect();
    let docstring = format!("describe {name} as {}", fields_doc.join(", "));

    let i = PYTHON_INDENT;
    let mut lines = Vec::new();

    lines.push("@dataclass(frozen=True)".to_owned());
    lines.push(format!("class {name}:"));
    lines.push(format!("{i}\"\"\"{docstring}\"\"\""));

    // Emit fields.
    for field in &node.metadata.fields {
        let py_type = resolve_python_type(&field.type_ref, imports);
        lines.push(format!("{i}{}: {py_type}", field.name));
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

    fn make_describe_node(name: &str, fields: Vec<(&str, &str)>) -> Node {
        let mut node = Node {
            id: NodeId::new(),
            intent: format!("{name} record"),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: Vec::new(),
            metadata: NodeMetadata::default(),
        };
        node.metadata.name = Some(name.to_owned());
        node.metadata.fields = fields
            .into_iter()
            .map(|(n, t)| Field {
                name: n.to_owned(),
                type_ref: t.to_owned(),
            })
            .collect();
        node
    }

    #[test]
    fn emit_describe_user() {
        let node = make_describe_node(
            "User",
            vec![
                ("id", "UserId"),
                ("balance", "WalletBalance"),
                ("status", "UserStatus"),
            ],
        );
        let mut imports = ImportSet::new();
        let result = emit_describe_node(&node, &mut imports).unwrap();

        assert!(result.contains("@dataclass(frozen=True)"));
        assert!(result.contains("class User:"));
        assert!(result.contains("id: UserId"));
        assert!(result.contains("balance: WalletBalance"));
        assert!(result.contains("status: UserStatus"));
        assert!(imports.needs_dataclass);
    }

    #[test]
    fn emit_describe_with_primitive_fields() {
        let node = make_describe_node(
            "Config",
            vec![("name", "text"), ("count", "integer"), ("rate", "number")],
        );
        let mut imports = ImportSet::new();
        let result = emit_describe_node(&node, &mut imports).unwrap();

        assert!(result.contains("name: str"));
        assert!(result.contains("count: int"));
        assert!(result.contains("rate: float"));
    }

    #[test]
    fn emit_describe_field_order_preserved() {
        let node = make_describe_node(
            "Ordered",
            vec![
                ("z_last", "text"),
                ("a_first", "text"),
                ("m_middle", "text"),
            ],
        );
        let mut imports = ImportSet::new();
        let result = emit_describe_node(&node, &mut imports).unwrap();

        let z_pos = result.find("z_last").unwrap();
        let a_pos = result.find("a_first").unwrap();
        let m_pos = result.find("m_middle").unwrap();
        assert!(z_pos < a_pos, "z_last should appear before a_first");
        assert!(a_pos < m_pos, "a_first should appear before m_middle");
    }

    #[test]
    fn emit_describe_missing_name_returns_error() {
        let mut node = make_describe_node("X", vec![]);
        node.metadata.name = None;
        let mut imports = ImportSet::new();
        let err = emit_describe_node(&node, &mut imports).unwrap_err();
        assert!(matches!(err, EmitError::DescribeNodeMissingName { .. }));
    }

    #[test]
    fn emit_describe_docstring_includes_fields() {
        let node = make_describe_node("Pair", vec![("x", "integer"), ("y", "integer")]);
        let mut imports = ImportSet::new();
        let result = emit_describe_node(&node, &mut imports).unwrap();

        assert!(result.contains("\"\"\"describe Pair as x:integer, y:integer\"\"\""));
    }
}
