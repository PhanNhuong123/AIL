use std::collections::{BTreeMap, HashMap, HashSet};

use ail_contract::VerifiedGraph;
use ail_graph::{ContractKind, GraphBackend, Node, NodeId, Pattern};

use crate::types::{EmitConfig, EmitOutput, EmittedFile, FileOwnership, TestFramework};
use crate::typescript::fn_name::to_camel_case_fn;
use crate::typescript::import_tracker::TypeKind;
use crate::typescript::type_map::{is_primitive_type, to_snake_case};

// ── Public entry point ─────────────────────────────────────────────────────────

/// Generate TypeScript test stub files for all top-level `do` nodes.
///
/// Produces one `tests/{snake}.test.ts` per top-level Do node. Each file
/// contains `it.todo()` stubs for:
/// - Happy path (always)
/// - Precondition violations (one per `promise before` contract)
/// - Boundary cases (one per `promise before` contract)
/// - Postcondition (one per real `promise after` contract, not dummy padding)
/// - Invariants (one per `promise always` contract)
/// - Type constraints (one per param whose type is a `define`-derived branded type)
/// - Error paths (one per unique error type referenced in `check...otherwise raise`)
///
/// Uses `config.test_framework` to select the import source (`vitest` or `@jest/globals`).
///
/// Returns an empty `EmitOutput` when no top-level Do nodes exist.
pub fn emit_ts_test_definitions(verified: &VerifiedGraph, config: &EmitConfig) -> EmitOutput {
    let graph = verified.graph();
    let all_nodes = graph.all_nodes_vec();

    // Build type registry: name → TypeKind.
    let mut type_registry: HashMap<String, TypeKind> = HashMap::new();
    for node in &all_nodes {
        if let Some(name) = &node.metadata.name {
            let kind = match node.pattern {
                Pattern::Define => TypeKind::Define,
                Pattern::Describe => TypeKind::Describe,
                Pattern::Error => TypeKind::Error,
                _ => continue,
            };
            type_registry.insert(name.clone(), kind);
        }
    }

    let mut files: Vec<EmittedFile> = Vec::new();

    for node in &all_nodes {
        if node.pattern != Pattern::Do {
            continue;
        }
        // Skip nested Do nodes (parent is also Do).
        let parent_is_do = graph
            .parent(node.id)
            .ok()
            .flatten()
            .and_then(|pid| graph.get_node(pid).ok().flatten())
            .map(|p| p.pattern == Pattern::Do)
            .unwrap_or(false);
        if parent_is_do {
            continue;
        }

        let raw_name = match node.metadata.name.as_deref() {
            Some(n) => n,
            None => continue,
        };
        let snake = to_snake_case(&raw_name.replace(' ', "_"));
        let fn_name = to_camel_case_fn(raw_name);

        let content =
            render_test_file(graph, node, &fn_name, &snake, config, &type_registry);
        files.push(EmittedFile {
            path: format!("tests/{snake}.test.ts"),
            content,
            ownership: FileOwnership::Generated,
        });
    }

    EmitOutput { files }
}

// ── File renderer ──────────────────────────────────────────────────────────────

fn render_test_file(
    graph: &dyn GraphBackend,
    node: &Node,
    fn_name: &str,
    snake: &str,
    config: &EmitConfig,
    type_registry: &HashMap<String, TypeKind>,
) -> String {
    // ── Collect stubs material ────────────────────────────────────────────────
    // Real Before contracts — skip dummy padding (`true == true`) added by test helpers or
    // the graph builder to satisfy validation rule v005.
    let before_exprs: Vec<&str> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Before && c.expression.0 != "true == true")
        .map(|c| c.expression.0.as_str())
        .collect();

    // Real After contracts — skip dummy padding added by the graph builder.
    let after_exprs: Vec<&str> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::After && c.expression.0 != "true == true")
        .map(|c| c.expression.0.as_str())
        .collect();

    let always_exprs: Vec<&str> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Always)
        .map(|c| c.expression.0.as_str())
        .collect();

    let check_errors = collect_check_errors(graph, node.id);

    // Params with Define types → type-constraint stubs.
    let define_params: Vec<(&str, &str)> = node
        .metadata
        .params
        .iter()
        .filter(|p| type_registry.get(&p.type_ref) == Some(&TypeKind::Define))
        .map(|p| (p.name.as_str(), p.type_ref.as_str()))
        .collect();

    // ── Build import table ─────────────────────────────────────────────────────
    // BTreeMap<module_path, Vec<symbol>> for deterministic output.
    let mut imports: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for param in &node.metadata.params {
        let type_ref = &param.type_ref;
        if is_primitive_type(type_ref) {
            continue;
        }
        if let Some(&kind) = type_registry.get(type_ref.as_str()) {
            let type_snake = to_snake_case(type_ref);
            let module = match kind {
                TypeKind::Define | TypeKind::Describe => format!("../types/{type_snake}"),
                TypeKind::Error => format!("../errors/{type_snake}"),
            };
            let syms = imports.entry(module).or_default();
            if !syms.contains(type_ref) {
                syms.push(type_ref.clone());
            }
            if kind == TypeKind::Define {
                let factory = format!("create{type_ref}");
                if !syms.contains(&factory) {
                    syms.push(factory);
                }
            }
        }
    }

    for err_type in &check_errors {
        let err_snake = to_snake_case(err_type);
        let module = format!("../errors/{err_snake}");
        let syms = imports.entry(module).or_default();
        if !syms.contains(err_type) {
            syms.push(err_type.clone());
        }
    }

    // ── Render lines ──────────────────────────────────────────────────────────
    let framework_source = match config.test_framework {
        TestFramework::Vitest => "vitest",
        TestFramework::Jest => "@jest/globals",
    };

    let mut lines: Vec<String> = vec![
        "// Generated test stubs — implement test bodies before running.".to_owned(),
        format!("// {framework_source} run tests/{snake}.test.ts"),
        format!("import {{ describe, it, expect }} from '{framework_source}';"),
        format!("import {{ {fn_name} }} from '../fn/{snake}';"),
    ];

    for (module, syms) in &imports {
        let mut sorted = syms.clone();
        sorted.sort();
        lines.push(format!(
            "import {{ {} }} from '{}';",
            sorted.join(", "),
            module
        ));
    }

    lines.push(String::new());
    lines.push(format!("describe('{fn_name}', () => {{"));

    // Happy path — always present.
    lines.push("  // --- Happy path ---".to_owned());
    lines.push("  it.todo('should succeed with valid inputs');".to_owned());

    // Precondition violation + boundary — one pair per Before contract.
    for expr in &before_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Precondition: {expr} ---"));
        lines.push(format!(
            "  it.todo('should throw when precondition violated: {expr}');"
        ));
        lines.push(format!("  it.todo('boundary: {expr}');"));
    }

    // Postcondition stubs — one per real After contract.
    for expr in &after_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Postcondition: {expr} ---"));
        lines.push(format!(
            "  it.todo('should satisfy postcondition: {expr}');"
        ));
    }

    // Invariant stubs — one per Always contract.
    for expr in &always_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Invariant: {expr} ---"));
        lines.push(format!("  it.todo('should maintain invariant: {expr}');"));
    }

    // Type-constraint stubs — one per Define-typed param.
    for (param_name, type_name) in &define_params {
        lines.push(String::new());
        lines.push(format!("  // --- Type constraint: {param_name} is {type_name} ---"));
        lines.push(format!(
            "  it.todo('should reject when {param_name} violates {type_name} constraint');"
        ));
    }

    // Error-path stubs — one per unique error type from Check children.
    for err_type in &check_errors {
        lines.push(String::new());
        lines.push(format!("  // --- Error path: {err_type} ---"));
        lines.push(format!(
            "  it.todo('should throw {err_type} when check fails');"
        ));
    }

    lines.push("});".to_owned());
    lines.push(String::new());

    lines.join("\n")
}

// ── Check-error collection ─────────────────────────────────────────────────────

/// Walk a node's children recursively and collect unique error type names
/// from `Check` nodes that have `metadata.otherwise_error` set.
fn collect_check_errors(graph: &dyn GraphBackend, node_id: NodeId) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    collect_check_errors_recursive(graph, node_id, &mut errors, &mut seen);
    errors
}

fn collect_check_errors_recursive(
    graph: &dyn GraphBackend,
    node_id: NodeId,
    errors: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    let node = match graph.get_node(node_id).ok().flatten() {
        Some(n) => n,
        None => return,
    };

    if node.pattern == Pattern::Check {
        if let Some(err_type) = &node.metadata.otherwise_error {
            if !err_type.is_empty() && !seen.contains(err_type.as_str()) {
                seen.insert(err_type.clone());
                errors.push(err_type.clone());
            }
        }
    }

    if let Some(children) = &node.children {
        for &child_id in children {
            collect_check_errors_recursive(graph, child_id, errors, seen);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_snake_case_spaces() {
        assert_eq!(to_snake_case(&"transfer money".replace(' ', "_")), "transfer_money");
    }

    #[test]
    fn framework_import_vitest() {
        let config = EmitConfig::default();
        assert_eq!(config.test_framework, TestFramework::Vitest);
    }

    #[test]
    fn framework_import_jest() {
        let config = EmitConfig {
            test_framework: TestFramework::Jest,
            ..Default::default()
        };
        assert_eq!(config.test_framework, TestFramework::Jest);
    }
}
