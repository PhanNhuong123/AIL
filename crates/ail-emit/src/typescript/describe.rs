use ail_graph::Node;

use crate::errors::EmitError;
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::type_map::{collect_user_types, resolve_ts_type, to_snake_case};

const I: &str = "  ";

/// Emit a TypeScript `interface` + factory function for a Describe node.
///
/// `describe T as f1:T1, f2:T2` becomes:
/// - `export interface T { readonly f1: T1; readonly f2: T2; }`
/// - `export function createT(params: { f1: T1; f2: T2 }): T`
pub(crate) fn emit_describe_node(
    node: &Node,
    tracker: &mut ImportTracker,
    type_registry: &std::collections::HashMap<String, TypeKind>,
) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsDescribeNodeMissingName { node_id: node.id })?;

    // Docstring.
    let fields_doc: Vec<_> = node
        .metadata
        .fields
        .iter()
        .map(|f| format!("{}:{}", f.name, f.type_ref))
        .collect();
    let docstring = if fields_doc.is_empty() {
        format!("describe {name}")
    } else {
        format!("describe {name} as {}", fields_doc.join(", "))
    };

    // Register imports for user-defined field types.
    for field in &node.metadata.fields {
        for user_type in collect_user_types(&field.type_ref) {
            let snake = to_snake_case(&user_type);
            let kind = type_registry
                .get(&user_type)
                .copied()
                .unwrap_or(TypeKind::Describe);
            // Determine which subfolder the type lives in.
            let module = if kind == TypeKind::Error {
                format!("../errors/{snake}")
            } else {
                format!("./{snake}")
            };
            tracker.register(&user_type, &module, kind);
        }
    }

    let mut lines = Vec::new();

    // Interface.
    lines.push(format!("/** {docstring} */"));
    lines.push(format!("export interface {name} {{"));
    for field in &node.metadata.fields {
        let ts_type = resolve_ts_type(&field.type_ref);
        lines.push(format!("{I}readonly {}: {ts_type};", field.name));
    }
    lines.push("}".to_owned());
    lines.push(String::new());

    // Factory function.
    lines.push(format!("export function create{name}(params: {{"));
    for field in &node.metadata.fields {
        let ts_type = resolve_ts_type(&field.type_ref);
        lines.push(format!("{I}{}: {ts_type};", field.name));
    }
    lines.push(format!("}}): {name} {{"));
    lines.push(format!("{I}return Object.freeze({{ ...params }});"));
    lines.push("}".to_owned());

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;
    use std::collections::HashMap;

    fn make_describe(name: &str, fields: Vec<(&str, &str)>) -> Node {
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
    fn ts_describe_emits_interface() {
        let node = make_describe("User", vec![("id", "text"), ("balance", "number")]);
        let mut tracker = ImportTracker::new();
        let result = emit_describe_node(&node, &mut tracker, &HashMap::new()).unwrap();
        assert!(result.contains("export interface User {"));
        assert!(result.contains("readonly id: string;"));
        assert!(result.contains("readonly balance: number;"));
    }

    #[test]
    fn ts_describe_emits_factory() {
        let node = make_describe("User", vec![("id", "text")]);
        let mut tracker = ImportTracker::new();
        let result = emit_describe_node(&node, &mut tracker, &HashMap::new()).unwrap();
        assert!(result.contains("export function createUser(params: {"));
        assert!(result.contains("Object.freeze({ ...params })"));
    }

    #[test]
    fn ts_describe_missing_name_errors() {
        let mut node = make_describe("X", vec![]);
        node.metadata.name = None;
        let mut tracker = ImportTracker::new();
        let err = emit_describe_node(&node, &mut tracker, &HashMap::new()).unwrap_err();
        assert!(matches!(err, EmitError::TsDescribeNodeMissingName { .. }));
    }

    #[test]
    fn ts_describe_field_order_preserved() {
        let node = make_describe(
            "Ordered",
            vec![
                ("z_last", "text"),
                ("a_first", "text"),
                ("m_middle", "text"),
            ],
        );
        let mut tracker = ImportTracker::new();
        let result = emit_describe_node(&node, &mut tracker, &HashMap::new()).unwrap();
        let z = result.find("z_last").unwrap();
        let a = result.find("a_first").unwrap();
        let m = result.find("m_middle").unwrap();
        assert!(z < a && a < m);
    }
}
