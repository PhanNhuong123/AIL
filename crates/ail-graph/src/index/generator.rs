use crate::graph::AilGraph;
use crate::types::{NodeId, Pattern};

use super::entry::{ContractSummary, IndexEntry, IndexKind};
use super::folder_index::FolderIndex;

/// Generate a [`FolderIndex`] for the given folder node.
///
/// Collects all named, indexable declarations in the node's direct children
/// and — recursively — in any sub-folder children. `Do` nodes act as scope
/// boundaries: their signatures are included, but their implementation
/// sub-steps are not.
pub fn generate_folder_index_for_node(folder_id: NodeId, graph: &AilGraph) -> FolderIndex {
    let folder_name = graph
        .get_node(folder_id)
        .ok()
        .and_then(|n| n.metadata.name.clone())
        .unwrap_or_default();

    let entries = collect_declarations_in_folder(folder_id, graph);

    FolderIndex {
        folder_name,
        folder_node_id: folder_id,
        entries,
    }
}

/// Walk the direct children of `folder_id`, collecting indexable entries.
///
/// Recurses into non-`Do` children that have their own children (sub-folders).
/// Does NOT recurse into `Do` children — their body is implementation detail.
fn collect_declarations_in_folder(folder_id: NodeId, graph: &AilGraph) -> Vec<IndexEntry> {
    let children = match graph.children_of(folder_id) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut entries = Vec::new();

    for child_id in children {
        let child = match graph.get_node(child_id) {
            Ok(n) => n,
            Err(_) => continue,
        };

        if let Some(entry) = build_entry_from_node(child_id, graph) {
            entries.push(entry);
        }

        // Recurse into sub-folder children but never into Do function bodies.
        let is_boundary = matches!(child.pattern, Pattern::Do);
        let has_children = graph
            .children_of(child_id)
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if !is_boundary && has_children {
            entries.extend(collect_declarations_in_folder(child_id, graph));
        }
    }

    entries
}

/// Try to build an [`IndexEntry`] from a single node.
///
/// Returns `None` for unnamed nodes or patterns that are never indexed
/// (e.g., `Let`, `Check`, `Promise`).
fn build_entry_from_node(node_id: NodeId, graph: &AilGraph) -> Option<IndexEntry> {
    let node = graph.get_node(node_id).ok()?;
    let name = node.metadata.name.clone()?;

    let kind = match node.pattern {
        Pattern::Define => IndexKind::Type {
            base_type: node.metadata.base_type.clone(),
            constraint_expr: node.expression.as_ref().map(|e| e.0.clone()),
            fields: vec![],
        },
        Pattern::Describe => IndexKind::Type {
            base_type: None,
            constraint_expr: None,
            fields: node.metadata.fields.clone(),
        },
        Pattern::Error => IndexKind::ErrorType {
            carries: node.metadata.carries.clone(),
        },
        Pattern::Do => IndexKind::Function {
            params: node.metadata.params.clone(),
            return_type: node.metadata.return_type.clone(),
            contracts: node
                .contracts
                .iter()
                .map(|c| ContractSummary {
                    kind: c.kind.clone(),
                    expression: c.expression.0.clone(),
                })
                .collect(),
        },
        // All other patterns are implementation-level and never indexed.
        _ => return None,
    };

    Some(IndexEntry { name, kind, node_id })
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Contract, ContractKind, EdgeKind, Expression, Field, Node, NodeId, Param,
        Pattern,
    };
    use crate::AilGraph;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_node(id: NodeId, pattern: Pattern, name: Option<&str>) -> Node {
        let mut n = Node::new(id, "intent", pattern);
        n.metadata.name = name.map(str::to_string);
        n
    }

    fn make_define_node(id: NodeId, name: &str, base_type: &str) -> Node {
        let mut n = make_node(id, Pattern::Define, Some(name));
        n.metadata.base_type = Some(base_type.to_string());
        n
    }

    fn make_define_node_with_constraint(
        id: NodeId,
        name: &str,
        base_type: &str,
        constraint: &str,
    ) -> Node {
        let mut n = make_define_node(id, name, base_type);
        n.expression = Some(Expression(constraint.to_string()));
        n
    }

    fn make_describe_node(id: NodeId, name: &str, fields: Vec<Field>) -> Node {
        let mut n = make_node(id, Pattern::Describe, Some(name));
        n.metadata.fields = fields;
        n
    }

    fn make_error_node(id: NodeId, name: &str, carries: Vec<Field>) -> Node {
        let mut n = make_node(id, Pattern::Error, Some(name));
        n.metadata.carries = carries;
        n
    }

    fn make_do_node(id: NodeId, name: &str) -> Node {
        let mut n = make_node(id, Pattern::Do, Some(name));
        n.metadata.params = vec![Param {
            name: "x".to_string(),
            type_ref: "Amount".to_string(),
        }];
        n.metadata.return_type = Some("Invoice".to_string());
        n.contracts = vec![Contract {
            kind: ContractKind::Before,
            expression: Expression("x > 0".to_string()),
        }];
        n
    }

    fn field(name: &str, type_ref: &str) -> Field {
        Field {
            name: name.to_string(),
            type_ref: type_ref.to_string(),
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn index001_define_node_produces_type_entry_with_base_type() {
        let id = NodeId::new();
        let node = make_define_node(id, "Amount", "number");
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        let entry = build_entry_from_node(id, &graph).unwrap();
        assert_eq!(entry.name, "Amount");
        assert!(
            matches!(&entry.kind, IndexKind::Type { base_type: Some(bt), .. } if bt == "number")
        );
    }

    #[test]
    fn index002_define_node_with_constraint_includes_expr() {
        let id = NodeId::new();
        let node = make_define_node_with_constraint(id, "Amount", "number", "value >= 0");
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        let entry = build_entry_from_node(id, &graph).unwrap();
        assert!(
            matches!(&entry.kind, IndexKind::Type { constraint_expr: Some(c), .. } if c == "value >= 0")
        );
    }

    #[test]
    fn index003_describe_node_produces_type_entry_with_fields() {
        let id = NodeId::new();
        let fields = vec![field("id", "text"), field("amount", "Amount")];
        let node = make_describe_node(id, "Invoice", fields.clone());
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        let entry = build_entry_from_node(id, &graph).unwrap();
        assert_eq!(entry.name, "Invoice");
        assert!(matches!(&entry.kind, IndexKind::Type { fields: f, .. } if f == &fields));
    }

    #[test]
    fn index004_error_node_produces_error_entry_with_carries() {
        let id = NodeId::new();
        let carries = vec![field("id", "text")];
        let node = make_error_node(id, "NotFound", carries.clone());
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        let entry = build_entry_from_node(id, &graph).unwrap();
        assert_eq!(entry.name, "NotFound");
        assert!(matches!(&entry.kind, IndexKind::ErrorType { carries: c } if c == &carries));
    }

    #[test]
    fn index005_do_node_produces_function_entry_with_contracts() {
        let id = NodeId::new();
        let node = make_do_node(id, "create");
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        let entry = build_entry_from_node(id, &graph).unwrap();
        assert_eq!(entry.name, "create");
        assert!(matches!(
            &entry.kind,
            IndexKind::Function { contracts, .. } if contracts.len() == 1
        ));
    }

    #[test]
    fn index006_unnamed_node_excluded_from_index() {
        let id = NodeId::new();
        let node = make_node(id, Pattern::Define, None);
        let mut graph = AilGraph::new();
        graph.add_node(node).unwrap();

        assert!(build_entry_from_node(id, &graph).is_none());
    }

    #[test]
    fn index007_non_indexable_pattern_excluded() {
        for pattern in [Pattern::Let, Pattern::Check, Pattern::Promise] {
            let id = NodeId::new();
            let node = make_node(id, pattern, Some("x"));
            let mut graph = AilGraph::new();
            graph.add_node(node).unwrap();

            assert!(
                build_entry_from_node(id, &graph).is_none(),
                "expected None for {:?}",
                graph.get_node(id).unwrap().pattern
            );
        }
    }

    #[test]
    fn index008_do_node_children_excluded_from_folder_index() {
        // root → [do_fn → [sub_step]]
        // The folder index of root should include do_fn but NOT sub_step.
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let do_id = NodeId::new();
        let sub_id = NodeId::new();

        let mut root = Node::new(root_id, "root", Pattern::Define);
        root.metadata.name = Some("root".to_string());
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let do_node = make_do_node(do_id, "transfer");
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        // sub_step is a child of the Do node (implementation detail)
        let sub = make_do_node(sub_id, "validate");
        graph.add_node(sub).unwrap();
        graph.add_edge(do_id, sub_id, EdgeKind::Ev).unwrap();

        let index = generate_folder_index_for_node(root_id, &graph);
        let names: Vec<&str> = index.entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"transfer"), "expected 'transfer' in index");
        assert!(
            !names.contains(&"validate"),
            "'validate' must not appear — it is inside a Do body"
        );
    }

    #[test]
    fn index009_subfolder_contents_collected_recursively() {
        // root → [module (Describe) → [Amount (Define)]]
        // module is non-Do with children → should be recursed into.
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let module_id = NodeId::new();
        let amount_id = NodeId::new();

        let mut root = Node::new(root_id, "root", Pattern::Define);
        root.metadata.name = Some("root".to_string());
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        // module is a Describe node that has children (acts as sub-folder)
        let module = make_describe_node(module_id, "types_module", vec![]);
        graph.add_node(module).unwrap();
        graph.add_edge(root_id, module_id, EdgeKind::Ev).unwrap();

        let amount = make_define_node(amount_id, "Amount", "number");
        graph.add_node(amount).unwrap();
        graph.add_edge(module_id, amount_id, EdgeKind::Ev).unwrap();

        let index = generate_folder_index_for_node(root_id, &graph);
        let names: Vec<&str> = index.entries.iter().map(|e| e.name.as_str()).collect();

        assert!(
            names.contains(&"Amount"),
            "'Amount' must be collected from nested sub-folder"
        );
    }
}
