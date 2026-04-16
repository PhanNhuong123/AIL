use ail_graph::Node;

use crate::errors::EmitError;
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::type_map::{collect_user_types, resolve_ts_type, to_snake_case};

const I: &str = "  ";

/// Emit a TypeScript `class extends Error` for an Error node.
///
/// `error E carries f1:T1, f2:T2` becomes:
/// ```typescript
/// export class E extends Error {
///   readonly f1: T1;
///   readonly f2: T2;
///   constructor(params: { f1: T1; f2: T2 }) { ... }
/// }
/// ```
pub(crate) fn emit_error_node(
    node: &Node,
    tracker: &mut ImportTracker,
    type_registry: &std::collections::HashMap<String, TypeKind>,
) -> Result<String, EmitError> {
    let name = node
        .metadata
        .name
        .as_deref()
        .ok_or(EmitError::TsErrorNodeMissingName { node_id: node.id })?;

    // Docstring.
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

    // Register imports for user-defined carry types.
    for field in &node.metadata.carries {
        for user_type in collect_user_types(&field.type_ref) {
            let snake = to_snake_case(&user_type);
            let kind = type_registry
                .get(&user_type)
                .copied()
                .unwrap_or(TypeKind::Describe);
            let module = if kind == TypeKind::Error {
                format!("./{snake}")
            } else {
                format!("../types/{snake}")
            };
            tracker.register(&user_type, &module, kind);
        }
    }

    let mut lines = Vec::new();

    lines.push(format!("/** {docstring} */"));
    lines.push(format!("export class {name} extends Error {{"));

    // Readonly field declarations.
    for field in &node.metadata.carries {
        let ts_type = resolve_ts_type(&field.type_ref);
        lines.push(format!("{I}readonly {}: {ts_type};", field.name));
    }

    if node.metadata.carries.is_empty() {
        // Minimal class body — just override the name.
        lines.push(String::new());
        lines.push(format!("{I}constructor() {{"));
        lines.push(format!("{I}{I}super('{name}');"));
        lines.push(format!("{I}{I}this.name = '{name}';"));
        lines.push(format!("{I}}}"));
    } else {
        lines.push(String::new());

        // Constructor parameter type.
        lines.push(format!("{I}constructor(params: {{"));
        for field in &node.metadata.carries {
            let ts_type = resolve_ts_type(&field.type_ref);
            lines.push(format!("{I}{I}{}: {ts_type};", field.name));
        }
        lines.push(format!("{I}}}}}) {{"));

        // super() message.
        let msg_parts: Vec<_> = node
            .metadata
            .carries
            .iter()
            .map(|f| format!("{}=${{params.{}}}", f.name, f.name))
            .collect();
        lines.push(format!("{I}{I}super(`{name}: {}`);", msg_parts.join(", ")));
        lines.push(format!("{I}{I}this.name = '{name}';"));

        // Assign fields.
        for field in &node.metadata.carries {
            lines.push(format!("{I}{I}this.{n} = params.{n};", n = field.name));
        }

        lines.push(format!("{I}}}"));
    }

    lines.push("}".to_owned());

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;
    use std::collections::HashMap;

    fn make_error(name: &str, carries: Vec<(&str, &str)>) -> Node {
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
    fn ts_error_emits_class_extends_error() {
        let node = make_error("InsufficientBalanceError", vec![]);
        let mut tracker = ImportTracker::new();
        let result = emit_error_node(&node, &mut tracker, &HashMap::new()).unwrap();
        assert!(result.contains("export class InsufficientBalanceError extends Error {"));
    }

    #[test]
    fn ts_error_carries_fields_in_constructor() {
        let node = make_error(
            "InsufficientBalanceError",
            vec![
                ("current_balance", "number"),
                ("requested_amount", "number"),
            ],
        );
        let mut tracker = ImportTracker::new();
        let result = emit_error_node(&node, &mut tracker, &HashMap::new()).unwrap();
        assert!(result.contains("current_balance: number;"));
        assert!(result.contains("requested_amount: number;"));
        assert!(result.contains("this.current_balance = params.current_balance;"));
    }

    #[test]
    fn ts_error_message_includes_field_values() {
        let node = make_error(
            "InsufficientBalanceError",
            vec![("current_balance", "number")],
        );
        let mut tracker = ImportTracker::new();
        let result = emit_error_node(&node, &mut tracker, &HashMap::new()).unwrap();
        assert!(result.contains(
            "super(`InsufficientBalanceError: current_balance=${params.current_balance}`);"
        ));
    }

    #[test]
    fn ts_error_missing_name_errors() {
        let mut node = make_error("X", vec![]);
        node.metadata.name = None;
        let mut tracker = ImportTracker::new();
        let err = emit_error_node(&node, &mut tracker, &HashMap::new()).unwrap_err();
        assert!(matches!(err, EmitError::TsErrorNodeMissingName { .. }));
    }
}
