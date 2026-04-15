use ail_contract::VerifiedGraph;
use ail_graph::{ContractKind, Pattern};

use crate::python::function::slugify_name;
use crate::types::{EmitConfig, EmittedFile, FileOwnership};

// ── Public entry point ────────────────────────────────────────────────────────

/// Generate pytest stub tests for all top-level `do` nodes that carry contracts.
///
/// Returns `None` if no top-level Do node has any contracts (nothing to generate).
/// Returns `generated/test_contracts.py` otherwise, regardless of `contract_mode`
/// — contract tests document intent even when runtime injection is `Off`.
///
/// Each Do function with contracts gets one `class TestXContracts` (pytest-collectable) with one
/// `test_before/after_contract_N` stub per contract. Each stub contains the
/// raw contract expression in its docstring and a `pytest.skip(...)` body,
/// indicating that the human must fill in test setup and assertions.
///
/// ## Pytest invocation
/// Run from the project root so that `generated/` is on the Python path:
/// ```text
/// PYTHONPATH=. pytest generated/test_contracts.py
/// ```
pub(crate) fn emit_test_file(
    verified: &VerifiedGraph,
    _config: &EmitConfig,
) -> Option<EmittedFile> {
    let graph = verified.graph();

    // Collect top-level Do nodes that have at least one contract.
    let fn_nodes: Vec<_> = graph
        .all_nodes()
        .filter(|n| n.pattern == Pattern::Do)
        .filter(|n| {
            let parent_pattern = graph
                .parent_of(n.id)
                .unwrap_or(None)
                .and_then(|pid| graph.get_node(pid).ok())
                .map(|p| p.pattern.clone());
            parent_pattern != Some(Pattern::Do)
        })
        .filter(|n| !n.contracts.is_empty())
        .collect();

    if fn_nodes.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = vec![
        "# Generated contract tests — run from project root:".to_owned(),
        "# PYTHONPATH=. pytest generated/test_contracts.py".to_owned(),
        "import pytest".to_owned(),
    ];

    // Collect import names for function imports.
    let fn_names: Vec<String> = fn_nodes
        .iter()
        .filter_map(|n| n.metadata.name.as_deref())
        .map(slugify_name)
        .collect();

    if !fn_names.is_empty() {
        lines.push(format!(
            "from generated.functions import {}",
            fn_names.join(", ")
        ));
    }

    for node in &fn_nodes {
        let raw_name = match node.metadata.name.as_deref() {
            Some(n) => n,
            None => continue,
        };
        let fn_name = slugify_name(raw_name);

        // Build the class name: TestXContracts in PascalCase.
        let class_name = pascal_case(&fn_name);

        // Build param summary line for docstrings.
        let param_summary = if node.metadata.params.is_empty() {
            String::new()
        } else {
            let ps: Vec<String> = node
                .metadata
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.type_ref))
                .collect();
            let ret = node.metadata.return_type.as_deref().unwrap_or("None");
            format!("        # Params: {} -> {}", ps.join(", "), ret)
        };

        lines.push(String::new());
        lines.push(String::new());
        lines.push(format!("class Test{class_name}Contracts:"));
        lines.push(format!(
            "    \"\"\"Generated contract tests for {fn_name}.\"\"\""
        ));

        for (i, contract) in node.contracts.iter().enumerate() {
            let kind_label = match contract.kind {
                ContractKind::Before => "before",
                ContractKind::After => "after",
                ContractKind::Always => "always",
            };
            let raw_expr = &contract.expression.0;

            lines.push(String::new());
            lines.push(format!("    def test_{kind_label}_contract_{i}(self):"));
            lines.push(format!(
                "        \"\"\"promise {kind_label}: {raw_expr}\"\"\""
            ));
            if !param_summary.is_empty() {
                lines.push(param_summary.clone());
            }
            lines.push(format!("        # assert {raw_expr}"));
            lines.push(
                "        pytest.skip(\"Generated stub \u{2014} implement test body\")".to_owned(),
            );
        }
    }

    lines.push(String::new());
    let content = lines.join("\n");

    Some(EmittedFile {
        path: "generated/test_contracts.py".to_owned(),
        content,
        ownership: FileOwnership::Generated,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a snake_case function name to PascalCase class name.
///
/// `"transfer_money"` → `"TransferMoney"`.
fn pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_contract::verify;
    use ail_graph::{
        validate_graph, AilGraph, Contract, ContractKind, EdgeKind, Expression, Node, NodeId,
        NodeMetadata, Param, Pattern,
    };
    use ail_types::type_check;

    /// Build a Do node, ensuring both Before and After contracts are present
    /// (validation requires both). Additional test-specific contracts are appended.
    fn make_do_with_contracts(
        name: &str,
        params: Vec<(&str, &str)>,
        extra_contracts: Vec<(ContractKind, &str)>,
    ) -> Node {
        // Validation requires at least one Before and one After on every Do node.
        let has_before = extra_contracts.iter().any(|(k, _)| *k == ContractKind::Before);
        let has_after = extra_contracts.iter().any(|(k, _)| *k == ContractKind::After);

        let mut contracts: Vec<Contract> = extra_contracts
            .into_iter()
            .map(|(kind, expr)| Contract {
                kind,
                expression: Expression(expr.to_owned()),
            })
            .collect();
        if !has_before {
            contracts.push(Contract {
                kind: ContractKind::Before,
                expression: Expression("true == true".to_owned()),
            });
        }
        if !has_after {
            contracts.push(Contract {
                kind: ContractKind::After,
                expression: Expression("true == true".to_owned()),
            });
        }

        let mut node = Node {
            id: NodeId::new(),
            intent: name.to_owned(),
            pattern: Pattern::Do,
            children: None,
            expression: None,
            contracts,
            metadata: NodeMetadata::default(),
        };
        node.metadata.name = Some(name.to_owned());
        node.metadata.params = params
            .into_iter()
            .map(|(n, t)| Param {
                name: n.to_owned(),
                type_ref: t.to_owned(),
            })
            .collect();
        // Use a recognised AIL primitive to avoid UnresolvedTypeReference errors.
        node.metadata.return_type = Some("number".to_owned());
        node
    }

    fn build_verified(do_node: Node) -> ail_contract::VerifiedGraph {
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
        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();
        let valid = validate_graph(graph).unwrap();
        let typed = type_check(valid, &[]).unwrap();
        verify(typed).unwrap()
    }

    /// Build a verified graph with no Do nodes (types only) to test the None case.
    fn build_verified_no_do() -> ail_contract::VerifiedGraph {
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
        verify(typed).unwrap()
    }

    #[test]
    fn test_gen_returns_none_when_no_contracts() {
        // A graph with no Do nodes has no contracts to generate tests for.
        let verified = build_verified_no_do();
        let config = EmitConfig::default();
        assert!(emit_test_file(&verified, &config).is_none());
    }

    #[test]
    fn test_gen_produces_class_per_function() {
        let do_node = make_do_with_contracts(
            "transfer_money",
            vec![("amount", "number")],
            vec![(ContractKind::Before, "amount > 0")],
        );
        let verified = build_verified(do_node);
        let config = EmitConfig::default();
        let file = emit_test_file(&verified, &config).unwrap();
        assert!(file.content.contains("class TestTransferMoneyContracts:"));
        assert!(file.content.contains("def test_before_contract_0"));
    }

    #[test]
    fn test_gen_before_contract_in_docstring() {
        // "amount > 0" references a function parameter — must supply a matching param.
        let do_node = make_do_with_contracts(
            "pay",
            vec![("amount", "number")],
            vec![(ContractKind::Before, "amount > 0")],
        );
        let verified = build_verified(do_node);
        let config = EmitConfig::default();
        let file = emit_test_file(&verified, &config).unwrap();
        assert!(file.content.contains("promise before: amount > 0"));
    }

    #[test]
    fn test_gen_after_contract_stub() {
        let do_node = make_do_with_contracts(
            "pay",
            vec![],
            vec![(ContractKind::After, "result > 0")],
        );
        let verified = build_verified(do_node);
        let config = EmitConfig::default();
        let file = emit_test_file(&verified, &config).unwrap();
        // The node has the explicit after + a dummy before (index 0), so after is at index 1.
        assert!(file.content.contains("def test_after_contract_"));
        assert!(file.content.contains("pytest.skip"));
    }

    #[test]
    fn pascal_case_converts_snake() {
        assert_eq!(pascal_case("transfer_money"), "TransferMoney");
        assert_eq!(pascal_case("compute"), "Compute");
        assert_eq!(pascal_case("a_b_c"), "ABC");
    }
}
