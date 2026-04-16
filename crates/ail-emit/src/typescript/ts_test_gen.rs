use std::collections::{BTreeMap, HashMap, HashSet};

use ail_contract::VerifiedGraph;
use ail_graph::{ContractKind, GraphBackend, Node, NodeId, Param, Pattern};

use crate::types::{EmitConfig, EmitOutput, EmittedFile, FileOwnership, TestFramework};
use crate::typescript::fn_name::{detect_is_async, to_camel_case_fn};
use crate::typescript::import_tracker::TypeKind;
use crate::typescript::type_map::{is_primitive_type, to_snake_case};

// ── Public entry point ─────────────────────────────────────────────────────────

/// Generate TypeScript test files for all top-level `do` nodes.
///
/// Produces one `tests/{snake}.test.ts` per top-level Do node with:
/// - Happy path: real test body with hardcoded v2.0 fixture values.
/// - Precondition violations: real test body using an intentional type cast.
/// - Boundary cases: `it.todo` stub (boundary derivation is v3.0 work).
/// - Postcondition / invariant: real test body with a weak `toBeDefined()` assertion.
/// - Type constraints: real test body exercising the factory's validation check.
/// - Error paths: `it.skip` stub (reliably triggering checks requires constraint solving).
///
/// Fixture strategy (v2.0): hardcoded values — `createT(1)` for `define`-derived branded
/// types, primitive literals (`1`, `"test"`, `true`) for base types.  The generated
/// comment says "adjust if constraint requires a different value".
///
/// Uses `config.test_framework` to select the import source (`vitest` or `@jest/globals`).
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

        let content = render_test_file(graph, node, &fn_name, &snake, config, &type_registry);
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
    let is_async = detect_is_async(graph, node);

    // ── Collect contract expressions ──────────────────────────────────────────
    // Real Before contracts — skip dummy padding (`true == true`).
    let before_exprs: Vec<&str> = node
        .contracts
        .iter()
        .filter(|c| c.kind == ContractKind::Before && c.expression.0 != "true == true")
        .map(|c| c.expression.0.as_str())
        .collect();

    // Real After contracts — skip dummy padding.
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

    // Params with Define types → type-constraint tests.
    let define_params: Vec<(&str, &str)> = node
        .metadata
        .params
        .iter()
        .filter(|p| type_registry.get(&p.type_ref) == Some(&TypeKind::Define))
        .map(|p| (p.name.as_str(), p.type_ref.as_str()))
        .collect();

    // ── Build import table ─────────────────────────────────────────────────────
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
        "// Generated by ail build — test bodies use hardcoded fixtures (v2.0).".to_owned(),
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
    lines.extend(render_happy_path(
        fn_name,
        &node.metadata.params,
        type_registry,
        is_async,
    ));

    // Precondition violation + boundary — one pair per Before contract.
    for expr in &before_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Precondition: {expr} ---"));
        lines.extend(render_precondition_violation(
            expr,
            &node.metadata.params,
            type_registry,
            fn_name,
            is_async,
        ));
        lines.push(format!("  it.todo('boundary: {expr}');"));
    }

    // Postcondition — one per real After contract.
    for expr in &after_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Postcondition: {expr} ---"));
        lines.extend(render_postcondition(
            expr,
            &node.metadata.params,
            type_registry,
            fn_name,
            is_async,
        ));
    }

    // Invariant — one per Always contract.
    for expr in &always_exprs {
        lines.push(String::new());
        lines.push(format!("  // --- Invariant: {expr} ---"));
        lines.extend(render_invariant(
            expr,
            &node.metadata.params,
            type_registry,
            fn_name,
            is_async,
        ));
    }

    // Type-constraint tests — one per Define-typed param.
    for (param_name, type_name) in &define_params {
        lines.push(String::new());
        lines.push(format!(
            "  // --- Type constraint: {param_name} is {type_name} ---"
        ));
        lines.extend(render_type_constraint(param_name, type_name));
    }

    // Error-path stubs (it.skip) — one per unique error type from Check children.
    for err_type in &check_errors {
        lines.push(String::new());
        lines.push(format!("  // --- Error path: {err_type} ---"));
        lines.extend(render_error_path_skip(err_type, is_async));
    }

    lines.push("});".to_owned());
    lines.push(String::new());

    lines.join("\n")
}

// ── Body renderers ─────────────────────────────────────────────────────────────

/// Render a fixture value expression for a single param type.
///
/// - `define`-derived branded types: `createTypeName(1)`
/// - `describe` object types: `undefined as unknown as TypeName` (uncommon as Do params in v2.0)
/// - Primitives: `1` / `"test"` / `true` by type name
fn fixture_for_param(type_ref: &str, type_registry: &HashMap<String, TypeKind>) -> String {
    match type_registry.get(type_ref) {
        Some(TypeKind::Define) => format!("create{type_ref}(1)"),
        Some(TypeKind::Describe) => format!("undefined as unknown as {type_ref}"),
        _ => match type_ref {
            "text" => "\"test\"".to_owned(),
            "boolean" => "true".to_owned(),
            _ => "1".to_owned(), // number, integer, and unknown primitives
        },
    }
}

/// Render the comma-separated argument list using param names (which were bound via `const`).
fn call_args(params: &[Param]) -> String {
    params
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a happy-path test with fixture creation and a weak `toBeDefined()` assertion.
fn render_happy_path(
    fn_name: &str,
    params: &[Param],
    type_registry: &HashMap<String, TypeKind>,
    is_async: bool,
) -> Vec<String> {
    let modifier = if is_async { "async " } else { "" };
    let mut lines = vec![format!(
        "  it('should succeed with valid inputs', {modifier}() => {{"
    )];
    for p in params {
        let val = fixture_for_param(&p.type_ref, type_registry);
        lines.push(format!(
            "    const {} = {}; // test fixture — adjust if constraint requires a different value",
            p.name, val
        ));
    }
    let call = format!("{}({})", fn_name, call_args(params));
    if is_async {
        lines.push(format!("    const result = await {call};"));
    } else {
        lines.push(format!("    const result = {call};"));
    }
    lines.push("    expect(result).toBeDefined();".to_owned());
    lines.push("  });".to_owned());
    lines
}

/// Render a precondition-violation test.
///
/// Identifies the first param mentioned in the expression and passes a violation
/// value (`0` for primitives, `0 as unknown as T` for branded types).  Other
/// params receive normal fixture values inlined in the call arguments.
fn render_precondition_violation(
    expr: &str,
    params: &[Param],
    type_registry: &HashMap<String, TypeKind>,
    fn_name: &str,
    is_async: bool,
) -> Vec<String> {
    // Find the first param name that appears as a word in the expression.
    let violate_name: Option<&str> = params
        .iter()
        .find(|p| expr.split_whitespace().any(|tok| tok == p.name.as_str()))
        .map(|p| p.name.as_str())
        .or_else(|| params.first().map(|p| p.name.as_str()));

    let args: Vec<String> = params
        .iter()
        .map(|p| {
            if Some(p.name.as_str()) == violate_name {
                if type_registry.get(&p.type_ref) == Some(&TypeKind::Define) {
                    format!("0 as unknown as {}", p.type_ref)
                } else {
                    "0".to_owned()
                }
            } else {
                // Inline fixture for non-violated params.
                fixture_for_param(&p.type_ref, type_registry)
            }
        })
        .collect();

    let call = format!("{}({})", fn_name, args.join(", "));
    let modifier = if is_async { "async " } else { "" };
    let mut lines = vec![format!(
        "  it('should throw when precondition violated: {expr}', {modifier}() => {{"
    )];
    lines.push("    // Intentional cast: testing precondition enforcement at runtime".to_owned());
    if is_async {
        lines.push(format!("    await expect(() => {call}).rejects.toThrow();"));
    } else {
        lines.push(format!("    expect(() => {call}).toThrow();"));
    }
    lines.push("  });".to_owned());
    lines
}

/// Render a postcondition test with a weak `toBeDefined()` assertion.
fn render_postcondition(
    expr: &str,
    params: &[Param],
    type_registry: &HashMap<String, TypeKind>,
    fn_name: &str,
    is_async: bool,
) -> Vec<String> {
    let modifier = if is_async { "async " } else { "" };
    let mut lines = vec![format!(
        "  it('should satisfy postcondition: {expr}', {modifier}() => {{"
    )];
    lines.push(
        "    // v2.0: weak assertion — verifies the function completes without error".to_owned(),
    );
    for p in params {
        let val = fixture_for_param(&p.type_ref, type_registry);
        lines.push(format!(
            "    const {} = {}; // test fixture — adjust if constraint requires a different value",
            p.name, val
        ));
    }
    let call = format!("{}({})", fn_name, call_args(params));
    if is_async {
        lines.push(format!("    const result = await {call};"));
    } else {
        lines.push(format!("    const result = {call};"));
    }
    lines.push("    expect(result).toBeDefined();".to_owned());
    lines.push("  });".to_owned());
    lines
}

/// Render an invariant test with a weak `toBeDefined()` assertion.
fn render_invariant(
    expr: &str,
    params: &[Param],
    type_registry: &HashMap<String, TypeKind>,
    fn_name: &str,
    is_async: bool,
) -> Vec<String> {
    let modifier = if is_async { "async " } else { "" };
    let mut lines = vec![format!(
        "  it('should maintain invariant: {expr}', {modifier}() => {{"
    )];
    lines.push(
        "    // v2.0: weak assertion — verifies the function completes without error".to_owned(),
    );
    for p in params {
        let val = fixture_for_param(&p.type_ref, type_registry);
        lines.push(format!(
            "    const {} = {}; // test fixture — adjust if constraint requires a different value",
            p.name, val
        ));
    }
    let call = format!("{}({})", fn_name, call_args(params));
    if is_async {
        lines.push(format!("    const result = await {call};"));
    } else {
        lines.push(format!("    const result = {call};"));
    }
    lines.push("    expect(result).toBeDefined();".to_owned());
    lines.push("  });".to_owned());
    lines
}

/// Render a type-constraint violation test using the `create*` factory with `-1`.
///
/// `-1` violates most positive numeric constraints; the comment tells developers
/// to adjust if their constraint uses a different bound.
fn render_type_constraint(param_name: &str, type_name: &str) -> Vec<String> {
    vec![
        format!("  it('should reject when {param_name} violates {type_name} constraint', () => {{"),
        "    // -1 violates most positive numeric constraints — adjust if needed".to_owned(),
        format!("    expect(() => create{type_name}(-1)).toThrow();"),
        "  });".to_owned(),
    ]
}

/// Render an error-path test as `it.skip`.
///
/// Reliably triggering a `check...otherwise raise` condition requires constraint
/// solving (v3.0 work).  `it.skip` is honest: the test appears in reports as
/// skipped, can be filled in manually, and does not pollute the green-test set.
fn render_error_path_skip(err_type: &str, is_async: bool) -> Vec<String> {
    let modifier = if is_async { "async " } else { "" };
    let comment_call = if is_async {
        format!("    // await expect(() => fn(...)).rejects.toThrow({err_type});")
    } else {
        format!("    // expect(() => fn(...)).toThrow({err_type});")
    };
    vec![
        format!("  it.skip('should throw {err_type} when check fails', {modifier}() => {{"),
        "    // TODO: set up fixture values that trigger this check condition, then assert:"
            .to_owned(),
        comment_call,
        "  });".to_owned(),
    ]
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
        assert_eq!(
            to_snake_case(&"transfer money".replace(' ', "_")),
            "transfer_money"
        );
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

    #[test]
    fn fixture_for_define_returns_factory() {
        let mut reg = HashMap::new();
        reg.insert("WalletBalance".to_owned(), TypeKind::Define);
        assert_eq!(
            fixture_for_param("WalletBalance", &reg),
            "createWalletBalance(1)"
        );
    }

    #[test]
    fn fixture_for_primitive_number() {
        let reg = HashMap::new();
        assert_eq!(fixture_for_param("number", &reg), "1");
    }

    #[test]
    fn fixture_for_primitive_text() {
        let reg = HashMap::new();
        assert_eq!(fixture_for_param("text", &reg), "\"test\"");
    }

    #[test]
    fn fixture_for_primitive_boolean() {
        let reg = HashMap::new();
        assert_eq!(fixture_for_param("boolean", &reg), "true");
    }
}
