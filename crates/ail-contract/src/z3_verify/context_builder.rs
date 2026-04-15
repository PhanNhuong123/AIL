use ail_graph::{
    types::{Node, Pattern},
    GraphBackend,
};
use ail_types::BuiltinSemanticType;

use crate::z3_encode::EncodeContext;

use super::sort::{sort_for_type_ref, Z3Sort};

/// Build a Z3 [`EncodeContext`] from a `Do` node's parameter list plus its
/// return type.
///
/// **Registered variables:**
/// 1. Each param whose type maps to a scalar Z3 sort (Int, Real, Bool) is
///    registered both as a current-state var and as an old-state var (needed
///    for `old()` references in postconditions).
/// 2. The synthetic `result` variable, if the return type maps to a scalar sort.
/// 3. `result.{field}` paths for each scalar field of a `Describe` return type
///    (postconditions typically reference fields, not the aggregate `result`).
///
/// `Uninterpreted` sorts (record types, text types, unknown types) are silently
/// skipped. No type constraint is asserted for those variables.
pub(super) fn build_encode_context<'ctx>(
    node: &Node,
    graph: &dyn GraphBackend,
    z3_ctx: &'ctx z3::Context,
) -> EncodeContext<'ctx> {
    let mut enc = EncodeContext::new(z3_ctx);

    // ── 1. Params ─────────────────────────────────────────────────────────────
    for param in &node.metadata.params {
        match sort_for_type_ref(&param.type_ref, graph) {
            Z3Sort::Int => {
                enc.add_int_var(&param.name);
                enc.add_old_int_var(&param.name);
            }
            Z3Sort::Real => {
                enc.add_real_var(&param.name);
                enc.add_old_real_var(&param.name);
            }
            Z3Sort::Bool => {
                enc.add_bool_var(&param.name);
                enc.add_old_bool_var(&param.name);
            }
            Z3Sort::Uninterpreted => {
                // Record or text type — no Z3 scalar representation.
                // If the type is a Describe node, expand its scalar fields
                // so expressions like "param.balance" can be encoded.
                expand_record_fields(&mut enc, &param.name, &param.type_ref, graph);
            }
        }
    }

    // ── 2. Result variable ────────────────────────────────────────────────────
    if let Some(ret_type) = &node.metadata.return_type {
        match sort_for_type_ref(ret_type, graph) {
            Z3Sort::Int => {
                enc.add_int_var("result");
            }
            Z3Sort::Real => {
                enc.add_real_var("result");
            }
            Z3Sort::Bool => {
                enc.add_bool_var("result");
            }
            Z3Sort::Uninterpreted => {
                // Describe return type: expand scalar fields as "result.{field}".
                expand_record_fields(&mut enc, "result", ret_type, graph);
            }
        }
    }

    enc
}

/// Collect the type constraints that should be asserted for each non-Uninterpreted
/// param in `node`.
///
/// Returns a list of `(variable_name, BuiltinSemanticType)` pairs for params
/// whose `type_ref` resolves to a builtin semantic type. Callers assert the
/// corresponding Z3 assertions via [`encode_type_constraint`].
pub(super) fn collect_param_type_constraints(
    node: &Node,
    graph: &dyn GraphBackend,
) -> Vec<(String, BuiltinSemanticType)> {
    let mut out = Vec::new();

    for param in &node.metadata.params {
        if let Some(builtin) = BuiltinSemanticType::from_name(&param.type_ref) {
            // Only numeric builtins produce Z3-encodable constraints.
            match builtin {
                BuiltinSemanticType::PositiveInteger
                | BuiltinSemanticType::NonNegativeInteger
                | BuiltinSemanticType::PositiveAmount
                | BuiltinSemanticType::Percentage => out.push((param.name.clone(), builtin)),
                _ => {}
            }
        } else {
            // User-defined alias: check if the Define resolves to a builtin constraint.
            // We look for a Define node whose base_type is a builtin name.
            if let Some(resolved) = resolve_define_to_builtin(&param.type_ref, graph, 0) {
                out.push((param.name.clone(), resolved));
            }
        }
    }

    out
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Scan the graph for a `Describe` node whose `metadata.name == type_name`.
/// For each scalar field, register a `"{prefix}.{field.name}"` Z3 variable.
fn expand_record_fields(
    enc: &mut EncodeContext<'_>,
    prefix: &str,
    type_name: &str,
    graph: &dyn GraphBackend,
) {
    let Some(describe) = find_describe_node(graph, type_name) else {
        return;
    };

    for field in &describe.metadata.fields {
        let path = format!("{prefix}.{}", field.name);
        match sort_for_type_ref(&field.type_ref, graph) {
            Z3Sort::Int => {
                enc.add_int_var(&path);
                enc.add_old_int_var(&path);
            }
            Z3Sort::Real => {
                enc.add_real_var(&path);
                enc.add_old_real_var(&path);
            }
            Z3Sort::Bool => {
                enc.add_bool_var(&path);
                enc.add_old_bool_var(&path);
            }
            Z3Sort::Uninterpreted => {
                // Nested record or text field — recurse one level.
                expand_record_fields(enc, &path, &field.type_ref, graph);
            }
        }
    }
}

/// Find the first `Describe` node in `graph` whose `metadata.name` matches `type_name`.
///
/// Returns an owned `Node` because `all_nodes_vec()` allocates; callers that previously
/// held `&'g Node` borrows must now hold `Node` values.
pub(super) fn find_describe_node(
    graph: &dyn GraphBackend,
    type_name: &str,
) -> Option<ail_graph::types::Node> {
    graph
        .all_nodes_vec()
        .into_iter()
        .find(|n| n.pattern == Pattern::Describe && n.metadata.name.as_deref() == Some(type_name))
}

/// Walk `Define` nodes in the graph to check if `type_ref` eventually aliases a
/// builtin semantic type. Returns `None` if the chain ends at a non-builtin or if
/// no matching `Define` node exists.
fn resolve_define_to_builtin(
    type_ref: &str,
    graph: &dyn GraphBackend,
    depth: u8,
) -> Option<BuiltinSemanticType> {
    const MAX: u8 = 8;
    if depth >= MAX {
        return None;
    }

    for node in graph.all_nodes_vec() {
        if node.pattern != Pattern::Define {
            continue;
        }
        if node.metadata.name.as_deref() != Some(type_ref) {
            continue;
        }
        let base = node.metadata.base_type.as_deref()?;
        if let Some(builtin) = BuiltinSemanticType::from_name(base) {
            return Some(builtin);
        }
        return resolve_define_to_builtin(base, graph, depth + 1);
    }

    None
}
