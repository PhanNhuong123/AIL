use std::collections::{HashMap, HashSet};

use ail_graph::{GraphBackend, Node, NodeId, Param, Pattern};

use crate::errors::EmitError;
use crate::python::using::collect_required_phases;
use crate::types::EmitConfig;
use crate::typescript::fn_name::{detect_is_async, to_camel_case_fn};
use crate::typescript::import_tracker::{ImportTracker, TypeKind};
use crate::typescript::ts_blocks::{emit_foreach_ts, emit_retry_ts, emit_together_ts};
use crate::typescript::ts_statement::{
    emit_check_ts, emit_fetch_ts, emit_let_ts, emit_match_ts, emit_raise_ts, emit_remove_ts,
    emit_return_ts, emit_save_ts, emit_update_ts,
};
use crate::typescript::type_map::{is_primitive_type, resolve_ts_type, to_snake_case};

// ── Public entry point ─────────────────────────────────────────────────────────

/// Emit a top-level `do` node as a TypeScript function.
///
/// Returns the complete function text (comment header + signature + body).
/// Helper functions for nested `do` nodes with different params are pushed
/// to `helpers` and should be prepended before this function in the file.
pub(crate) fn emit_ts_do_function(
    graph: &dyn GraphBackend,
    node: &Node,
    type_registry: &HashMap<String, TypeKind>,
    config: &EmitConfig,
    tracker: &mut ImportTracker,
    helpers: &mut Vec<String>,
) -> Result<String, Vec<EmitError>> {
    let raw_name = node
        .metadata
        .name
        .as_deref()
        .ok_or_else(|| vec![EmitError::TsDoNodeMissingName { node_id: node.id }])?;
    let fn_name = to_camel_case_fn(raw_name);

    let is_async = config.async_mode || detect_is_async(graph, node);

    // Register param and return types for imports.
    for p in &node.metadata.params {
        register_fn_import(tracker, &p.type_ref, type_registry);
    }
    if let Some(rt) = node.metadata.return_type.as_deref() {
        let primary = rt.split(" or ").next().unwrap_or(rt).trim();
        if !is_primitive_type(primary) && primary != "void" {
            register_fn_import(tracker, primary, type_registry);
        }
    }

    let mut lines = emit_function_header(
        &fn_name,
        &node.intent,
        &node.metadata.params,
        node.metadata.return_type.as_deref(),
        is_async,
    );

    let required_phases = collect_required_phases(graph, node.id);

    let body = if let Some(children) = &node.children {
        emit_ts_do_body(
            graph,
            node.id,
            &node.metadata.params,
            children,
            1,
            type_registry,
            is_async,
            tracker,
            &required_phases,
            helpers,
            None,
        )?
    } else {
        vec![]
    };

    if body.is_empty() {
        lines.push("  // no-op".to_owned());
    } else {
        lines.extend(body);
    }
    lines.push("}".to_owned());

    Ok(lines.join("\n"))
}

// ── Function header ────────────────────────────────────────────────────────────

fn emit_function_header(
    fn_name: &str,
    intent: &str,
    params: &[Param],
    return_type_raw: Option<&str>,
    is_async: bool,
) -> Vec<String> {
    let ts_ret = match return_type_raw {
        Some(rt) => ts_primary_return_type(rt),
        None => "void".to_owned(),
    };
    let wrapped = if is_async {
        if ts_ret == "void" {
            "Promise<void>".to_owned()
        } else {
            format!("Promise<{ts_ret}>")
        }
    } else {
        ts_ret
    };

    let akw = if is_async { "async " } else { "" };
    let mut lines = vec![format!("// [AIL] {fn_name}: {intent}")];

    if params.is_empty() {
        lines.push(format!("{akw}function {fn_name}(): {wrapped} {{"));
    } else if params.len() == 1 {
        let p = &params[0];
        let ts_t = resolve_ts_type(&p.type_ref);
        lines.push(format!(
            "{akw}function {fn_name}({}: {ts_t}): {wrapped} {{",
            p.name
        ));
    } else {
        lines.push(format!("{akw}function {fn_name}("));
        for p in params {
            let ts_t = resolve_ts_type(&p.type_ref);
            lines.push(format!("  {}: {ts_t},", p.name));
        }
        lines.push(format!("): {wrapped} {{"));
    }
    lines
}

fn ts_primary_return_type(type_ref: &str) -> String {
    // Errors after "or" become throws; only the primary type is the return.
    let primary = type_ref.split(" or ").next().unwrap_or(type_ref).trim();
    resolve_ts_type(primary)
}

// ── Body emitter ───────────────────────────────────────────────────────────────

/// Emit a `do` body recursively, dispatching each child to its emitter.
///
/// `tx_name`: when inside a `together` block, state ops use this name instead
/// of the parsed source, matching the spec's `tx.updateUser(...)` pattern.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_ts_do_body(
    graph: &dyn GraphBackend,
    parent_id: NodeId,
    parent_params: &[Param],
    children: &[NodeId],
    indent_level: usize,
    type_registry: &HashMap<String, TypeKind>,
    fn_async: bool,
    tracker: &mut ImportTracker,
    phase_markers: &HashSet<String>,
    helpers: &mut Vec<String>,
    tx_name: Option<&str>,
) -> Result<Vec<String>, Vec<EmitError>> {
    let _ = parent_id; // used for error messages in future
    let indent = "  ".repeat(indent_level);
    let mut lines: Vec<String> = Vec::new();
    let mut errors: Vec<EmitError> = Vec::new();
    let mut seen_phases: HashSet<String> = HashSet::new();

    for &child_id in children {
        let child_owned = match graph.get_node(child_id).ok().flatten() {
            Some(n) => n,
            None => continue,
        };
        let child = &child_owned;

        if let Some(ref name) = child.metadata.name {
            if phase_markers.contains(name.as_str()) && !seen_phases.contains(name) {
                lines.push(format!("{indent}// === [Phase: {name}] ==="));
                seen_phases.insert(name.clone());
            }
        }

        match child.pattern {
            Pattern::Do => {
                if params_equal(&child.metadata.params, parent_params) {
                    lines.push(format!("{indent}// --- {} ---", child.intent));
                    if let Some(gc) = &child.children {
                        match emit_ts_do_body(
                            graph,
                            child.id,
                            parent_params,
                            gc,
                            indent_level,
                            type_registry,
                            fn_async,
                            tracker,
                            &HashSet::new(),
                            helpers,
                            tx_name,
                        ) {
                            Ok(inner) => lines.extend(inner),
                            Err(errs) => errors.extend(errs),
                        }
                    }
                } else {
                    // Different params → emit as a private helper function.
                    let orig_fn_name = to_camel_case_fn(&child.intent);
                    let helper_name = format!("_{orig_fn_name}");
                    let child_async = fn_async || detect_is_async(graph, child);
                    let mut h_tracker = ImportTracker::new();
                    let mut h_helpers = Vec::new();
                    let child_config = EmitConfig {
                        async_mode: child_async,
                        ..Default::default()
                    };
                    match emit_ts_do_function(
                        graph,
                        child,
                        type_registry,
                        &child_config,
                        &mut h_tracker,
                        &mut h_helpers,
                    ) {
                        Ok(helper_fn) => {
                            // Rename function to underscore-prefixed private helper.
                            let renamed = helper_fn.replace(
                                &format!("function {orig_fn_name}"),
                                &format!("function {helper_name}"),
                            );
                            helpers.extend(h_helpers);
                            helpers.push(renamed);
                            lines.push(format!(
                                "{indent}// --- {} (helper: {helper_name}) ---",
                                child.intent
                            ));
                        }
                        Err(errs) => errors.extend(errs),
                    }
                }
            }
            Pattern::Let => match emit_let_ts(child, &indent, type_registry) {
                Ok(l) => lines.push(l),
                Err(e) => errors.push(e),
            },
            Pattern::Check => match emit_check_ts(child, &indent) {
                Ok(b) => lines.push(b),
                Err(e) => errors.push(e),
            },
            Pattern::Match => match emit_match_ts(child, &indent) {
                Ok(b) => lines.push(b),
                Err(e) => errors.push(e),
            },
            Pattern::Fetch => match emit_fetch_ts(child, &indent, tx_name) {
                Ok(l) => {
                    if let Some(ty) = child.metadata.return_type.as_deref() {
                        register_fn_import(tracker, ty, type_registry);
                    }
                    lines.push(l);
                }
                Err(e) => errors.push(e),
            },
            Pattern::Save => match emit_save_ts(child, &indent, tx_name) {
                Ok(l) => lines.push(l),
                Err(e) => errors.push(e),
            },
            Pattern::Update => match emit_update_ts(child, &indent, tx_name) {
                Ok(l) => {
                    if let Some(ty) = child.metadata.return_type.as_deref() {
                        register_fn_import(tracker, ty, type_registry);
                    }
                    lines.push(l);
                }
                Err(e) => errors.push(e),
            },
            Pattern::Remove => match emit_remove_ts(child, &indent, tx_name) {
                Ok(l) => {
                    if let Some(ty) = child.metadata.return_type.as_deref() {
                        register_fn_import(tracker, ty, type_registry);
                    }
                    lines.push(l);
                }
                Err(e) => errors.push(e),
            },
            Pattern::Return => match emit_return_ts(child, &indent, type_registry) {
                Ok(b) => {
                    if let Some(ty) = child.metadata.name.as_deref() {
                        register_fn_import(tracker, ty, type_registry);
                    }
                    lines.push(b);
                }
                Err(e) => errors.push(e),
            },
            Pattern::Raise => match emit_raise_ts(child, &indent) {
                Ok(l) => {
                    if let Some(ty) = child.metadata.name.as_deref() {
                        register_fn_import(tracker, ty, type_registry);
                    }
                    lines.push(l);
                }
                Err(e) => errors.push(e),
            },
            Pattern::ForEach => {
                match emit_foreach_ts(
                    graph,
                    child,
                    indent_level,
                    type_registry,
                    fn_async,
                    tracker,
                    parent_params,
                    helpers,
                ) {
                    Ok(b) => lines.extend(b),
                    Err(errs) => errors.extend(errs),
                }
            }
            Pattern::Together => {
                match emit_together_ts(
                    graph,
                    child,
                    indent_level,
                    type_registry,
                    tracker,
                    parent_params,
                    helpers,
                ) {
                    Ok(b) => lines.extend(b),
                    Err(errs) => errors.extend(errs),
                }
            }
            Pattern::Retry => {
                match emit_retry_ts(
                    graph,
                    child,
                    indent_level,
                    type_registry,
                    fn_async,
                    tracker,
                    parent_params,
                    helpers,
                ) {
                    Ok(b) => lines.extend(b),
                    Err(errs) => errors.extend(errs),
                }
            }
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(lines)
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Register a type name for import into a `fn/` file.
///
/// Types in `types/` (Define/Describe) → `../types/{snake}`
/// Types in `errors/` (Error) → `../errors/{snake}`
pub(crate) fn register_fn_import(
    tracker: &mut ImportTracker,
    type_name: &str,
    type_registry: &HashMap<String, TypeKind>,
) {
    if let Some(&kind) = type_registry.get(type_name) {
        let snake = to_snake_case(type_name);
        let module_path = match kind {
            TypeKind::Define | TypeKind::Describe => format!("../types/{snake}"),
            TypeKind::Error => format!("../errors/{snake}"),
        };
        tracker.register(type_name, &module_path, kind);
    }
}

/// Compare two param slices by name and type_ref.
pub(crate) fn params_equal(a: &[Param], b: &[Param]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| x.name == y.name && x.type_ref == y.type_ref)
}
