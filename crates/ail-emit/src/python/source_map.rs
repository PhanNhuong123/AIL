use ail_contract::VerifiedGraph;
use ail_graph::{ContractKind, Pattern};
use serde::Serialize;

use crate::python::function::slugify_name;
use crate::types::{EmittedFile, FileOwnership};

// ── Serialisable source-map types ─────────────────────────────────────────────

#[derive(Serialize)]
struct SourceMap {
    version: &'static str,
    generated: &'static str,
    functions: Vec<FunctionEntry>,
}

#[derive(Serialize)]
struct FunctionEntry {
    python_name: String,
    node_id: String,
    intent: String,
    params: Vec<ParamEntry>,
    return_type: String,
    contracts: Vec<ContractEntry>,
}

#[derive(Serialize)]
struct ParamEntry {
    name: String,
    type_ref: String,
}

#[derive(Serialize)]
struct ContractEntry {
    kind: String,
    expression: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Generate a function-level source map for all top-level `do` nodes.
///
/// Returns `None` if no top-level Do nodes exist in the verified graph.
/// Returns `generated/functions.ailmap.json` otherwise.
///
/// The map links each Python function name back to its AIL node id, intent,
/// parameter types, return type, and raw contract expressions.
///
/// ## v0.1 Limitation
/// Mapping is function-level only; no per-line offsets are tracked.
pub(crate) fn emit_source_map(verified: &VerifiedGraph) -> Option<EmittedFile> {
    let graph = verified.graph();

    let all_nodes = graph.all_nodes_vec();
    let functions: Vec<FunctionEntry> = all_nodes
        .into_iter()
        .filter(|n| n.pattern == Pattern::Do)
        .filter(|n| {
            let parent_pattern = graph
                .parent(n.id)
                .ok()
                .flatten()
                .and_then(|pid| graph.get_node(pid).ok().flatten())
                .map(|p| p.pattern.clone());
            parent_pattern != Some(Pattern::Do)
        })
        .filter_map(|n| {
            let raw_name = n.metadata.name.as_deref()?;
            Some(FunctionEntry {
                python_name: slugify_name(raw_name),
                node_id: n.id.to_string(),
                intent: n.intent.clone(),
                params: n
                    .metadata
                    .params
                    .iter()
                    .map(|p| ParamEntry {
                        name: p.name.clone(),
                        type_ref: p.type_ref.clone(),
                    })
                    .collect(),
                return_type: n
                    .metadata
                    .return_type
                    .clone()
                    .unwrap_or_else(|| "None".to_owned()),
                contracts: n
                    .contracts
                    .iter()
                    .map(|c| ContractEntry {
                        kind: match c.kind {
                            ContractKind::Before => "before",
                            ContractKind::After => "after",
                            ContractKind::Always => "always",
                        }
                        .to_owned(),
                        expression: c.expression.0.clone(),
                    })
                    .collect(),
            })
        })
        .collect();

    if functions.is_empty() {
        return None;
    }

    let map = SourceMap {
        version: "1.0",
        generated: "generated/functions.py",
        functions,
    };

    // Serialise with pretty-print JSON for readability.
    let content = serde_json::to_string_pretty(&map)
        .expect("SourceMap serialization is infallible for well-formed data");

    Some(EmittedFile {
        path: "generated/functions.ailmap.json".to_owned(),
        content,
        ownership: FileOwnership::Generated,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_contract::verify;
    use ail_graph::{
        validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
        NodeMetadata, Pattern,
    };
    use ail_types::type_check;

    /// Build a verified graph with a top-level Do node.
    ///
    /// Always ensures both Before and After contracts are present (validation
    /// requires both). If only one kind is supplied, a dummy `"true == true"`
    /// contract is added for the missing kind.
    fn build_verified_with_do(
        name: &str,
        contracts: Vec<(ContractKind, &str)>,
    ) -> ail_contract::VerifiedGraph {
        let has_before = contracts.iter().any(|(k, _)| *k == ContractKind::Before);
        let has_after = contracts.iter().any(|(k, _)| *k == ContractKind::After);

        let mut all = contracts;
        if !has_before {
            all.push((ContractKind::Before, "true == true"));
        }
        if !has_after {
            all.push((ContractKind::After, "true == true"));
        }

        let mut graph = AilGraph::new();
        let root = Node {
            id: NodeId::new(),
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        let root_id = root.id;
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let mut do_node = Node {
            id: NodeId::new(),
            intent: name.to_owned(),
            pattern: Pattern::Do,
            children: None,
            expression: None,
            contracts: all
                .into_iter()
                .map(|(kind, expr)| Contract {
                    kind,
                    expression: Expression(expr.to_owned()),
                })
                .collect(),
            metadata: NodeMetadata::default(),
        };
        do_node.metadata.name = Some(name.to_owned());
        // Use a recognised AIL primitive to avoid UnresolvedTypeReference errors.
        do_node.metadata.return_type = Some("number".to_owned());

        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        let valid = validate_graph(graph).unwrap();
        let typed = type_check(valid, &[]).unwrap();
        verify(typed).unwrap()
    }

    #[test]
    fn source_map_returns_none_when_no_do_nodes() {
        let mut graph = AilGraph::new();
        let root = Node {
            id: NodeId::new(),
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        let root_id = root.id;
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();
        let valid = validate_graph(graph).unwrap();
        let typed = type_check(valid, &[]).unwrap();
        let verified = verify(typed).unwrap();
        assert!(emit_source_map(&verified).is_none());
    }

    #[test]
    fn source_map_contains_python_name_and_node_id() {
        // Use "true == true" — no variable refs, so passes before-contract scope checks.
        let verified = build_verified_with_do(
            "transfer_money",
            vec![(ContractKind::Before, "true == true")],
        );
        let file = emit_source_map(&verified).unwrap();
        assert_eq!(file.path, "generated/functions.ailmap.json");
        assert!(file.content.contains("\"python_name\""));
        assert!(file.content.contains("\"transfer_money\""));
        assert!(file.content.contains("\"node_id\""));
    }

    #[test]
    fn source_map_lists_contracts() {
        let verified = build_verified_with_do(
            "pay",
            vec![
                (ContractKind::Before, "true == true"),
                (ContractKind::After, "true == true"),
            ],
        );
        let file = emit_source_map(&verified).unwrap();
        assert!(file.content.contains("\"before\""));
        assert!(file.content.contains("\"after\""));
        assert!(file.content.contains("true == true"));
    }

    #[test]
    fn source_map_version_is_1_0() {
        let verified = build_verified_with_do("foo", vec![(ContractKind::Before, "true == true")]);
        let file = emit_source_map(&verified).unwrap();
        assert!(file.content.contains("\"version\": \"1.0\""));
    }
}
