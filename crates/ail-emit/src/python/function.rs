use std::collections::HashSet;

use ail_graph::{GraphBackend, Node, NodeId, Pattern};

use crate::constants::PYTHON_INDENT;
use crate::errors::EmitError;
use crate::python::contract_inject::{
    collect_old_refs, render_after_contract_lines, render_before_contract_lines,
};
use crate::python::expression_parser::{parse_foreach_expression, parse_retry_expression};
use crate::python::statement::{
    emit_check, emit_fetch, emit_let, emit_match, emit_raise, emit_remove, emit_return, emit_save,
    emit_update,
};
use crate::python::type_map::resolve_python_type;
use crate::python::using::{collect_required_phases, emit_using_do};
use crate::types::{EmitConfig, ImportSet};

// ── Public helpers ────────────────────────────────────────────────────────────

/// Resolve a return type that may be a union: `"TypeA or TypeB"` → `"TypeA | TypeB"`.
///
/// Calls `resolve_python_type` on each alternative.
pub(crate) fn resolve_return_type(type_ref: &str, imports: &mut ImportSet) -> String {
    let parts: Vec<&str> = type_ref.split(" or ").collect();
    if parts.len() == 1 {
        return resolve_python_type(type_ref.trim(), imports);
    }
    parts
        .iter()
        .map(|t| resolve_python_type(t.trim(), imports))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Convert an intent string to a Python function name.
///
/// `"transfer money safely"` → `"transfer_money_safely"`.
/// Assumes ASCII-only intent strings (current AIL convention for generated names).
pub(crate) fn slugify_name(intent: &str) -> String {
    intent
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        // Collapse multiple underscores.
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

// ── Function emission ─────────────────────────────────────────────────────────

/// Emit a top-level `do` node as a Python function definition.
///
/// ```python
/// [async ]def {name}({params}) -> {return_type}:
///     """{intent}"""
///     # old() snapshots (if any after-contracts reference old())
///     # before-contracts (assert / comment / nothing per ContractMode)
///     {body}
/// ```
pub(crate) fn emit_do_function(
    graph: &dyn GraphBackend,
    node: &Node,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<String, Vec<EmitError>> {
    let raw_name = node
        .metadata
        .name
        .as_deref()
        .ok_or_else(|| vec![EmitError::DoNodeMissingName { node_id: node.id }])?;

    let fn_name = slugify_name(raw_name);

    // Build parameter list.
    let params: Vec<String> = node
        .metadata
        .params
        .iter()
        .map(|p| {
            let py_type = resolve_python_type(&p.type_ref, imports);
            format!("{}: {py_type}", p.name)
        })
        .collect();

    // Resolve return type.
    let return_type = node
        .metadata
        .return_type
        .as_deref()
        .map(|rt| resolve_return_type(rt, imports))
        .unwrap_or_else(|| "None".to_owned());

    let async_kw = if config.async_mode { "async " } else { "" };
    let params_str = params.join(", ");

    // Emit function header.
    let mut lines = vec![
        format!("{async_kw}def {fn_name}({params_str}) -> {return_type}:"),
        format!("{PYTHON_INDENT}\"\"\"{}\"\"\"", node.intent),
    ];

    // Collect old() snapshot assignments for after-contracts.
    let old_refs = collect_old_refs(node.id, &node.contracts).map_err(|e| vec![e])?;
    for (snapshot_name, source_expr) in &old_refs {
        lines.push(format!("{PYTHON_INDENT}{snapshot_name} = {source_expr}"));
    }

    // Emit before-contract assertions (Before + Always contracts).
    let before_lines = render_before_contract_lines(
        node.id,
        &node.contracts,
        PYTHON_INDENT,
        &config.contract_mode,
        imports,
    )
    .map_err(|e| vec![e])?;
    lines.extend(before_lines);

    // Pre-render after-contract assertions at indent level 1.
    // These will be injected immediately before every Pattern::Return node.
    let after_lines = render_after_contract_lines(
        node.id,
        &node.contracts,
        PYTHON_INDENT,
        &config.contract_mode,
        imports,
    )
    .map_err(|e| vec![e])?;

    // If this Do node uses a shared pattern, inline its body with substitutions
    // instead of emitting the node's own (absent) children.
    // using-Do bodies contain no explicit Return nodes, so after_lines are not threaded.
    if node.metadata.using_pattern_name.is_some() {
        let using_lines = emit_using_do(graph, node, 1, config, imports)?;
        if using_lines.is_empty() {
            lines.push(format!("{PYTHON_INDENT}pass"));
        } else {
            lines.extend(using_lines);
        }
        return Ok(lines.join("\n"));
    }

    // Collect required phase names from template refs (following pattern).
    let required_phases = collect_required_phases(graph, node.id);

    // Emit body from children, threading after_contracts so they are injected
    // before every Pattern::Return at any depth.
    if let Some(children) = &node.children {
        let body_lines = emit_do_body_phased(
            graph,
            node.id,
            children,
            1,
            config,
            imports,
            &required_phases,
            &after_lines,
        )?;
        if body_lines.is_empty() {
            lines.push(format!("{PYTHON_INDENT}pass"));
        } else {
            lines.extend(body_lines);
        }
    } else {
        lines.push(format!("{PYTHON_INDENT}pass"));
    }

    Ok(lines.join("\n"))
}

/// Emit the body of a `do` node recursively.
///
/// Processes each child node in order, dispatching to the appropriate emitter.
/// Returns a flat list of indented Python lines.
///
/// `after_contracts` are pre-rendered assertion lines (at `indent_level`'s
/// indentation) that must be injected immediately before every `Pattern::Return`.
///
/// This is the public variant used by block emitters that do not carry `following`
/// template refs. It delegates to `emit_do_body_phased` with an empty phase-marker set.
pub(crate) fn emit_do_body(
    graph: &dyn GraphBackend,
    children: &[NodeId],
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
    after_contracts: &[String],
) -> Result<Vec<String>, Vec<EmitError>> {
    emit_do_body_phased(
        graph,
        NodeId::default(),
        children,
        indent_level,
        config,
        imports,
        &HashSet::new(),
        after_contracts,
    )
}

/// Emit the body of a `do` node, injecting `# === [Phase: X] ===` comments
/// before children whose `metadata.name` is in `phase_markers`.
///
/// `parent_node_id` is used only for the `MissingTemplatePhase` error message.
/// `after_contracts` are injected immediately before each `Pattern::Return` child
/// (at this indent level and all nested depths — except inside `together` blocks).
#[allow(clippy::too_many_arguments)]
fn emit_do_body_phased(
    graph: &dyn GraphBackend,
    parent_node_id: NodeId,
    children: &[NodeId],
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
    phase_markers: &HashSet<String>,
    after_contracts: &[String],
) -> Result<Vec<String>, Vec<EmitError>> {
    let indent = PYTHON_INDENT.repeat(indent_level);
    let mut lines = Vec::new();
    let mut errors = Vec::new();
    let mut seen_phases: HashSet<String> = HashSet::new();

    for &child_id in children {
        let child_owned = match graph.get_node(child_id).ok().flatten() {
            Some(n) => n,
            // Logically impossible in a VerifiedGraph.
            None => continue,
        };
        let child = &child_owned;

        // Inject phase separator comment before matching children.
        if let Some(ref name) = child.metadata.name {
            if phase_markers.contains(name.as_str()) {
                lines.push(format!("{indent}# === [Phase: {name}] ==="));
                seen_phases.insert(name.clone());
            }
        }

        match child.pattern {
            // Nested Do → section comment + recurse (passes after_contracts through).
            Pattern::Do if child.metadata.using_pattern_name.is_some() => {
                match emit_using_do(graph, child, indent_level, config, imports) {
                    Ok(block_lines) => lines.extend(block_lines),
                    Err(errs) => errors.extend(errs),
                }
            }

            Pattern::Do => {
                lines.push(format!("{indent}# --- {} ---", child.intent));
                if let Some(grandchildren) = &child.children {
                    match emit_do_body(
                        graph,
                        grandchildren,
                        indent_level,
                        config,
                        imports,
                        after_contracts,
                    ) {
                        Ok(inner) => lines.extend(inner),
                        Err(errs) => errors.extend(errs),
                    }
                }
            }

            Pattern::Let => match emit_let(child, &indent, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Check => match emit_check(child, &indent, imports) {
                Ok(block) => lines.push(block),
                Err(e) => errors.push(e),
            },

            Pattern::ForEach => {
                match emit_foreach_block(
                    graph,
                    child,
                    indent_level,
                    config,
                    imports,
                    after_contracts,
                ) {
                    Ok(block_lines) => lines.extend(block_lines),
                    Err(errs) => errors.extend(errs),
                }
            }

            Pattern::Match => match emit_match(child, &indent, imports) {
                Ok(block) => lines.push(block),
                Err(e) => errors.push(e),
            },

            Pattern::Fetch => match emit_fetch(child, &indent, config, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Save => match emit_save(child, &indent, config, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Update => match emit_update(child, &indent, config, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Remove => match emit_remove(child, &indent, config, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Return => {
                // Inject after-contracts immediately before the return statement.
                for contract_line in after_contracts {
                    // Re-indent to match the current indent level.
                    let trimmed = contract_line.trim_start();
                    lines.push(format!("{indent}{trimmed}"));
                }
                match emit_return(child, &indent, imports) {
                    Ok(line) => lines.push(line),
                    Err(e) => errors.push(e),
                }
            }

            Pattern::Raise => match emit_raise(child, &indent, imports) {
                Ok(line) => lines.push(line),
                Err(e) => errors.push(e),
            },

            Pattern::Together => {
                // Pass &[] — after-contracts must NOT be injected inside an atomic
                // transaction scope (v0.1 documented limitation).
                match emit_together_block(graph, child, indent_level, config, imports) {
                    Ok(block_lines) => lines.extend(block_lines),
                    Err(errs) => errors.extend(errs),
                }
            }

            Pattern::Retry => {
                match emit_retry_block(graph, child, indent_level, config, imports, after_contracts)
                {
                    Ok(block_lines) => lines.extend(block_lines),
                    Err(errs) => errors.extend(errs),
                }
            }

            // Promise nodes: contract injection handled at function level above.
            // Define, Describe, Error: emitted by emit_types, not here.
            _ => {}
        }
    }

    // Defensive completeness check.
    if !phase_markers.is_empty() {
        for phase in phase_markers {
            if !seen_phases.contains(phase) {
                errors.push(EmitError::MissingTemplatePhase {
                    node_id: parent_node_id,
                    phase: phase.clone(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(lines)
}

// ── Block emitters ─────────────────────────────────────────────────────────────

/// Emit a `for each` loop.
///
/// `after_contracts` are threaded through so returns inside the loop body
/// also receive after-contract injection.
fn emit_foreach_block(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
    after_contracts: &[String],
) -> Result<Vec<String>, Vec<EmitError>> {
    let indent = PYTHON_INDENT.repeat(indent_level);

    let (var_name, collection) = if let Some(col) = &node.metadata.collection {
        let var = node.metadata.name.as_deref().unwrap_or("_item");
        (var.to_owned(), col.clone())
    } else if let Some(expr) = &node.expression {
        let (var, _ty, col, _intent) = parse_foreach_expression(&expr.0);
        (var, col)
    } else {
        ("_item".to_owned(), "_collection".to_owned())
    };

    let mut lines = vec![format!("{indent}for {var_name} in {collection}:")];

    if let Some(children) = &node.children {
        match emit_do_body(
            graph,
            children,
            indent_level + 1,
            config,
            imports,
            after_contracts,
        ) {
            Ok(body) => {
                if body.is_empty() {
                    let inner = PYTHON_INDENT.repeat(indent_level + 1);
                    lines.push(format!("{inner}pass"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        let inner = PYTHON_INDENT.repeat(indent_level + 1);
        lines.push(format!("{inner}pass"));
    }

    Ok(lines)
}

/// Emit a `together` block as an async transaction scope.
///
/// After-contracts are NOT threaded into the inner body (`&[]` is passed) because
/// injecting assertions inside an atomic transaction scope is incorrect (v0.1
/// documented limitation). After-contracts are injected at the enclosing
/// function's return point instead.
fn emit_together_block(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<Vec<String>, Vec<EmitError>> {
    let indent = PYTHON_INDENT.repeat(indent_level);

    imports.needs_asyncio = true;
    imports.needs_transaction = true;

    let mut lines = vec![format!("{indent}async with transaction():")];

    let async_config = EmitConfig {
        async_mode: true,
        ..Default::default()
    };
    let effective_config = if config.async_mode {
        config
    } else {
        &async_config
    };

    if let Some(children) = &node.children {
        // &[] — do not inject after-contracts inside a transaction block.
        match emit_do_body(
            graph,
            children,
            indent_level + 1,
            effective_config,
            imports,
            &[],
        ) {
            Ok(body) => {
                if body.is_empty() {
                    let inner = PYTHON_INDENT.repeat(indent_level + 1);
                    lines.push(format!("{inner}pass"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        let inner = PYTHON_INDENT.repeat(indent_level + 1);
        lines.push(format!("{inner}pass"));
    }

    Ok(lines)
}

/// Emit a `retry` block as a counted loop with try/except.
///
/// `after_contracts` are threaded through so returns inside the retry body
/// also receive after-contract injection.
fn emit_retry_block(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
    after_contracts: &[String],
) -> Result<Vec<String>, Vec<EmitError>> {
    let indent = PYTHON_INDENT.repeat(indent_level);
    let i1 = PYTHON_INDENT.repeat(indent_level + 1);
    let i2 = PYTHON_INDENT.repeat(indent_level + 2);
    let i3 = PYTHON_INDENT.repeat(indent_level + 3);

    let raw_expr = node
        .metadata
        .body_intent
        .as_deref()
        .or_else(|| node.expression.as_ref().map(|e| e.0.as_str()))
        .unwrap_or("1 times");
    let (count, delay_opt) = parse_retry_expression(raw_expr);
    let last_idx = count.saturating_sub(1);

    let delay_str = match &delay_opt {
        Some((val, unit)) => {
            let seconds = match unit.as_str() {
                "minute" | "min" => val * 60.0,
                "millisecond" | "ms" => val / 1000.0,
                _ => *val,
            };
            format!("{seconds:.1}")
        }
        None => "1.0".to_owned(),
    };

    let sleep_call = if config.async_mode {
        imports.needs_asyncio = true;
        format!("await asyncio.sleep({delay_str})")
    } else {
        imports.needs_time = true;
        format!("time.sleep({delay_str})")
    };

    let mut lines = vec![
        format!("{indent}for _retry_i in range({count}):"),
        format!("{i1}try:"),
    ];

    if let Some(children) = &node.children {
        match emit_do_body(
            graph,
            children,
            indent_level + 2,
            config,
            imports,
            after_contracts,
        ) {
            Ok(body) => {
                if body.is_empty() {
                    lines.push(format!("{i2}pass"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        lines.push(format!("{i2}pass"));
    }

    lines.push(format!("{i2}break"));
    lines.push(format!("{i1}except Exception:"));
    lines.push(format!("{i2}if _retry_i < {last_idx}:"));
    lines.push(format!("{i3}{sleep_call}"));
    lines.push(format!("{i2}else:"));
    lines.push(format!("{i3}raise"));

    Ok(lines)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ail_graph::*;

    #[test]
    fn slugify_name_space_separated() {
        assert_eq!(
            slugify_name("transfer money safely"),
            "transfer_money_safely"
        );
    }

    #[test]
    fn slugify_name_already_lowercase() {
        assert_eq!(slugify_name("compute"), "compute");
    }

    #[test]
    fn slugify_name_collapses_underscores() {
        assert_eq!(slugify_name("transfer  money"), "transfer_money");
    }

    #[test]
    fn resolve_return_type_single() {
        let mut imports = ImportSet::new();
        let result = resolve_return_type("TransferResult", &mut imports);
        assert_eq!(result, "TransferResult");
    }

    #[test]
    fn resolve_return_type_union() {
        let mut imports = ImportSet::new();
        let result =
            resolve_return_type("TransferResult or InsufficientBalanceError", &mut imports);
        assert_eq!(result, "TransferResult | InsufficientBalanceError");
    }

    #[test]
    fn resolve_return_type_primitive_union() {
        let mut imports = ImportSet::new();
        let result = resolve_return_type("number or text", &mut imports);
        assert_eq!(result, "float | str");
    }

    fn make_do_node(name: &str, params: Vec<(&str, &str)>, return_type: &str) -> Node {
        let mut node = Node {
            id: NodeId::new(),
            intent: name.to_owned(),
            pattern: Pattern::Do,
            children: None,
            expression: None,
            contracts: vec![],
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
        node.metadata.return_type = Some(return_type.to_owned());
        node
    }

    #[test]
    fn emit_do_sync_signature_format() {
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let root = Node {
            id: root_id,
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let do_node = make_do_node(
            "transfer_money",
            vec![("sender_id", "UserId"), ("amount", "PositiveAmount")],
            "TransferResult",
        );
        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        let do_node_ref = graph.get_node(do_id).unwrap();
        let config = EmitConfig {
            async_mode: false,
            ..Default::default()
        };
        let mut imports = ImportSet::new();
        let result = emit_do_function(&graph, do_node_ref, &config, &mut imports).unwrap();

        assert!(result.starts_with("def transfer_money("));
        assert!(result.contains("sender_id: UserId"));
        assert!(result.contains("amount: PositiveAmount"));
        assert!(result.contains("-> TransferResult:"));
        assert!(!result.contains("async def"));
    }

    #[test]
    fn emit_do_async_signature_format() {
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let root = Node {
            id: root_id,
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let do_node = make_do_node("transfer_money", vec![], "None");
        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        let do_node_ref = graph.get_node(do_id).unwrap();
        let config = EmitConfig {
            async_mode: true,
            ..Default::default()
        };
        let mut imports = ImportSet::new();
        let result = emit_do_function(&graph, do_node_ref, &config, &mut imports).unwrap();

        assert!(result.starts_with("async def transfer_money("));
    }

    #[test]
    fn emit_do_no_params() {
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let root = Node {
            id: root_id,
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let do_node = make_do_node("initialize", vec![], "InitResult");
        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        let do_node_ref = graph.get_node(do_id).unwrap();
        let config = EmitConfig {
            async_mode: false,
            ..Default::default()
        };
        let mut imports = ImportSet::new();
        let result = emit_do_function(&graph, do_node_ref, &config, &mut imports).unwrap();

        assert!(result.contains("def initialize() -> InitResult:"));
    }

    #[test]
    fn emit_do_union_return_type() {
        let mut graph = AilGraph::new();
        let root_id = NodeId::new();
        let root = Node {
            id: root_id,
            intent: "root".to_owned(),
            pattern: Pattern::Describe,
            children: None,
            expression: None,
            contracts: vec![],
            metadata: NodeMetadata::default(),
        };
        graph.add_node(root).unwrap();
        graph.set_root(root_id).unwrap();

        let do_node = make_do_node(
            "transfer_money",
            vec![],
            "TransferResult or InsufficientBalanceError",
        );
        let do_id = do_node.id;
        graph.add_node(do_node).unwrap();
        graph.add_edge(root_id, do_id, EdgeKind::Ev).unwrap();

        let do_node_ref = graph.get_node(do_id).unwrap();
        let config = EmitConfig {
            async_mode: false,
            ..Default::default()
        };
        let mut imports = ImportSet::new();
        let result = emit_do_function(&graph, do_node_ref, &config, &mut imports).unwrap();

        assert!(result.contains("-> TransferResult | InsufficientBalanceError:"));
    }
}
