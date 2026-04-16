use std::collections::{HashMap, HashSet};

use ail_graph::{GraphBackend, Node, Param, Pattern};

use crate::python::expression_parser::parse_retry_expression;
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::ts_function::emit_ts_do_body;

// ── ForEach ────────────────────────────────────────────────────────────────────

/// Emit a `for each` loop as `for (const item of collection) { body }`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_foreach_ts(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    type_registry: &HashMap<String, TypeKind>,
    fn_async: bool,
    tracker: &mut ImportTracker,
    parent_params: &[Param],
    helpers: &mut Vec<String>,
) -> Result<Vec<String>, Vec<EmitError>> {
    use crate::python::expression_parser::parse_foreach_expression;

    let indent = "  ".repeat(indent_level);
    let inner = "  ".repeat(indent_level + 1);

    let (var_name, _type_ref, collection) = {
        if let (Some(name), Some(col)) = (
            node.metadata.name.as_deref(),
            node.metadata.collection.as_deref(),
        ) {
            (name.to_owned(), String::new(), col.to_owned())
        } else if let Some(expr) = &node.expression {
            let (v, t, c, _) = parse_foreach_expression(&expr.0);
            (v, t, c)
        } else {
            ("_item".to_owned(), String::new(), "_collection".to_owned())
        }
    };

    let mut lines = vec![format!("{indent}for (const {var_name} of {collection}) {{")];

    if let Some(children) = &node.children {
        match emit_ts_do_body(
            graph,
            node.id,
            parent_params,
            children,
            indent_level + 1,
            type_registry,
            fn_async,
            tracker,
            &HashSet::new(),
            helpers,
            None,
        ) {
            Ok(body) => {
                if body.is_empty() {
                    lines.push(format!("{inner}// no-op"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        lines.push(format!("{inner}// no-op"));
    }

    lines.push(format!("{indent}}}"));
    Ok(lines)
}

// ── Together ───────────────────────────────────────────────────────────────────

/// Emit a `together` block as `await source.transaction(async (tx) => { body })`.
///
/// All state operations inside the block use `tx` as the repository proxy so
/// generated code matches the spec pattern. When children reference different
/// sources, a warning comment is emitted and the first source is used.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_together_ts(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    type_registry: &HashMap<String, TypeKind>,
    tracker: &mut ImportTracker,
    parent_params: &[Param],
    helpers: &mut Vec<String>,
) -> Result<Vec<String>, Vec<EmitError>> {
    let indent = "  ".repeat(indent_level);
    let inner = "  ".repeat(indent_level + 1);
    let source = collect_together_source(graph, node);

    let mut lines = vec![format!(
        "{indent}await {source}.transaction(async (tx) => {{"
    )];

    if let Some(children) = &node.children {
        match emit_ts_do_body(
            graph,
            node.id,
            parent_params,
            children,
            indent_level + 1,
            type_registry,
            true, // always async inside transaction
            tracker,
            &HashSet::new(),
            helpers,
            Some("tx"),
        ) {
            Ok(body) => {
                if body.is_empty() {
                    lines.push(format!("{inner}// no-op"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        lines.push(format!("{inner}// no-op"));
    }

    lines.push(format!("{indent}}});"));
    Ok(lines)
}

/// Collect the transaction source name from the first state-op child.
///
/// All state ops must share the same source for a clean transaction wrapper.
/// If they differ, a warning comment is emitted and the first source is used.
fn collect_together_source(graph: &dyn GraphBackend, node: &Node) -> String {
    use crate::python::expression_parser::{parse_fetch_expression, parse_update_expression};

    let children = match node.children.as_deref() {
        Some(c) => c,
        None => return "repository".to_owned(),
    };

    let mut sources: Vec<String> = Vec::new();

    for &child_id in children {
        let child = match graph.get_node(child_id).ok().flatten() {
            Some(n) => n,
            None => continue,
        };
        let expr = child
            .expression
            .as_ref()
            .map(|e| e.0.as_str())
            .unwrap_or("");
        let source = match child.pattern {
            Pattern::Fetch | Pattern::Remove => parse_fetch_expression(expr).0,
            Pattern::Update => parse_update_expression(expr).0,
            Pattern::Save => {
                // "to source [with ...]"
                expr.strip_prefix("to ")
                    .map(|s| s.split(' ').next().unwrap_or(s).to_owned())
                    .map(Some)
                    .unwrap_or(None)
            }
            _ => None,
        };
        if let Some(s) = source {
            if !sources.contains(&s) {
                sources.push(s);
            }
        }
    }

    match sources.len() {
        0 => "repository".to_owned(),
        1 => sources.remove(0),
        _ => {
            // Mixed sources — use first, caller will see warning comment in output
            sources.remove(0)
        }
    }
}

// ── Retry ──────────────────────────────────────────────────────────────────────

/// Emit a `retry` block as a counted loop with `setTimeout` delay.
///
/// Variables produced inside the retry body (from `let` bindings) should be
/// hoisted above the loop. For v2.0, a `let varName: unknown` declaration
/// is emitted above the loop when the body contains a `let` child.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_retry_ts(
    graph: &dyn GraphBackend,
    node: &Node,
    indent_level: usize,
    type_registry: &HashMap<String, TypeKind>,
    fn_async: bool,
    tracker: &mut ImportTracker,
    parent_params: &[Param],
    helpers: &mut Vec<String>,
) -> Result<Vec<String>, Vec<EmitError>> {
    use crate::typescript::fn_name::to_camel_case_var;
    use crate::typescript::type_map::resolve_ts_type;

    let indent = "  ".repeat(indent_level);
    let i1 = "  ".repeat(indent_level + 1);
    let i2 = "  ".repeat(indent_level + 2);

    let raw_expr = node
        .metadata
        .body_intent
        .as_deref()
        .or_else(|| node.expression.as_ref().map(|e| e.0.as_str()))
        .unwrap_or("1 times");
    let (count, delay_opt) = parse_retry_expression(raw_expr);
    let last_idx = count;

    let delay_ms = match &delay_opt {
        Some((val, unit)) => match unit.as_str() {
            "minute" | "min" => (*val * 60_000.0) as u64,
            "millisecond" | "ms" => *val as u64,
            _ => (*val * 1_000.0) as u64,
        },
        None => 1_000,
    };

    // Hoist any `let` variable declarations above the loop.
    let mut hoist_lines: Vec<String> = Vec::new();
    if let Some(children) = &node.children {
        for &child_id in children.iter() {
            let child = match graph.get_node(child_id).ok().flatten() {
                Some(n) => n,
                None => continue,
            };
            if child.pattern == Pattern::Let {
                if let Some(var_raw) = child.metadata.name.as_deref() {
                    let var = to_camel_case_var(var_raw);
                    let ts_type = child
                        .metadata
                        .return_type
                        .as_deref()
                        .map(resolve_ts_type)
                        .unwrap_or_else(|| "unknown".to_owned());
                    hoist_lines.push(format!("{indent}let {var}: {ts_type};"));
                }
            }
        }
    }

    let mut lines = hoist_lines;
    lines.push(format!(
        "{indent}for (let _attempt = 1; _attempt <= {count}; _attempt++) {{"
    ));
    lines.push(format!("{i1}try {{"));

    if let Some(children) = &node.children {
        match emit_ts_do_body(
            graph,
            node.id,
            parent_params,
            children,
            indent_level + 2,
            type_registry,
            fn_async,
            tracker,
            &HashSet::new(),
            helpers,
            None,
        ) {
            Ok(body) => {
                if body.is_empty() {
                    lines.push(format!("{i2}// no-op"));
                } else {
                    lines.extend(body);
                }
            }
            Err(errs) => return Err(errs),
        }
    } else {
        lines.push(format!("{i2}// no-op"));
    }

    lines.push(format!("{i2}break;"));
    lines.push(format!("{i1}}} catch (error) {{"));
    lines.push(format!("{i2}if (_attempt === {last_idx}) throw error;"));
    lines.push(format!(
        "{i2}await new Promise(resolve => setTimeout(resolve, {delay_ms}));"
    ));
    lines.push(format!("{i1}}}"));
    lines.push(format!("{indent}}}"));

    Ok(lines)
}

// Bring EmitError into scope for the type alias used internally.
use crate::errors::EmitError;
