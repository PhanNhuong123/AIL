use std::collections::HashMap;

use ail_graph::{ContextPacket, GraphBackend, NodeId, Pattern};

use crate::errors::TypeError;
use crate::expr::parse_constraint_expr;
use crate::types::{ConstraintExpr, ValueExpr};

use super::resolver::{find_type_node_by_name, resolve_type_ref};

// ─── Check 1 ─────────────────────────────────────────────────────────────────

/// Resolve every `type_ref` string stored in node metadata across the whole graph.
///
/// Checks: `metadata.params[*].type_ref`, `metadata.return_type`,
/// `metadata.base_type`, `metadata.fields[*].type_ref`,
/// `metadata.carries[*].type_ref`.
///
/// Emits [`TypeError::UndefinedType`] for each string that does not resolve.
pub(super) fn check_all_node_type_refs(graph: &dyn GraphBackend, errors: &mut Vec<TypeError>) {
    for node in graph.all_nodes_vec() {
        let id = node.id;

        for param in &node.metadata.params {
            if resolve_type_ref(&param.type_ref, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id: id,
                    name: param.type_ref.clone(),
                });
            }
        }

        if let Some(rt) = &node.metadata.return_type {
            if resolve_type_ref(rt, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id: id,
                    name: rt.clone(),
                });
            }
        }

        if let Some(bt) = &node.metadata.base_type {
            if resolve_type_ref(bt, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id: id,
                    name: bt.clone(),
                });
            }
        }

        for field in &node.metadata.fields {
            if resolve_type_ref(&field.type_ref, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id: id,
                    name: field.type_ref.clone(),
                });
            }
        }

        for field in &node.metadata.carries {
            if resolve_type_ref(&field.type_ref, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id: id,
                    name: field.type_ref.clone(),
                });
            }
        }
    }
}

// ─── Check 2 ─────────────────────────────────────────────────────────────────

/// Check that field-access paths in contract expressions resolve correctly.
///
/// For each node's contracts, parses the constraint expression and walks the
/// `Ref([a, b, c, ...])` paths. For each path:
/// - `a` must be a scope variable (if not found, skips — may be a quantifier
///   variable or a special reference like `value`).
/// - `b` must be a field on `a`'s type (if `a` is a `Describe` node).
/// - `c` must be a field on `b`'s type, and so on recursively.
///
/// Emits [`TypeError::UndefinedField`] when a field segment is missing.
///
/// Note: leaf node `expression` fields (value expressions like
/// `sender.balance - amount`) are not checked here — that is Phase 3 scope.
pub(super) fn check_contract_field_access(
    graph: &dyn GraphBackend,
    packets: &[ContextPacket],
    errors: &mut Vec<TypeError>,
) {
    let packet_by_node: HashMap<NodeId, &ContextPacket> =
        packets.iter().map(|p| (p.node_id, p)).collect();

    for node in graph.all_nodes_vec() {
        let Some(packet) = packet_by_node.get(&node.id) else {
            // No packet for this node — skip field-access checks.
            continue;
        };

        // Build a quick scope lookup: variable name → type_ref string.
        let scope_type: HashMap<&str, &str> = packet
            .scope
            .iter()
            .map(|sv| (sv.name.as_str(), sv.type_ref.as_str()))
            .collect();

        for contract in &node.contracts {
            // Parse the raw expression string. If parsing fails, skip silently —
            // the constraint evaluator (Phase 3) owns syntax error reporting.
            // A malformed expression neither produces field errors here nor
            // triggers a type error; it will surface as a parse/eval error
            // during verification.
            // TODO(phase-3): consider emitting a ParseWarning here so the
            // developer gets earlier feedback.
            let Ok(constraint) = parse_constraint_expr(contract.expression.0.as_str()) else {
                continue;
            };

            collect_constraint_field_errors(&constraint, &scope_type, graph, node.id, errors);
        }
    }
}

/// Walk a `ConstraintExpr` recursively and call
/// [`check_ref_chain`] for every `Ref` found inside it.
fn collect_constraint_field_errors(
    expr: &ConstraintExpr,
    scope_type: &HashMap<&str, &str>,
    graph: &dyn GraphBackend,
    node_id: NodeId,
    errors: &mut Vec<TypeError>,
) {
    match expr {
        ConstraintExpr::Compare { left, right, .. } => {
            collect_value_field_errors(left, scope_type, graph, node_id, errors);
            collect_value_field_errors(right, scope_type, graph, node_id, errors);
        }
        ConstraintExpr::In { value, collection } => {
            collect_value_field_errors(value, scope_type, graph, node_id, errors);
            collect_value_field_errors(collection, scope_type, graph, node_id, errors);
        }
        ConstraintExpr::Matches { value, .. } => {
            collect_value_field_errors(value, scope_type, graph, node_id, errors);
        }
        ConstraintExpr::And(children) | ConstraintExpr::Or(children) => {
            for child in children {
                collect_constraint_field_errors(child, scope_type, graph, node_id, errors);
            }
        }
        ConstraintExpr::Not(inner) => {
            collect_constraint_field_errors(inner, scope_type, graph, node_id, errors);
        }
        // Quantifier variables (e.g. `item` in `for all item in ...`) are bound
        // locally and not in the packet scope, so we skip the condition body to
        // avoid false-positive UndefinedField errors. The collection expression
        // IS resolved against the outer scope.
        ConstraintExpr::ForAll { collection, .. } | ConstraintExpr::Exists { collection, .. } => {
            collect_value_field_errors(collection, scope_type, graph, node_id, errors);
        }
    }
}

/// Walk a `ValueExpr` and call [`check_ref_chain`] for every `Ref` inside it.
fn collect_value_field_errors(
    expr: &ValueExpr,
    scope_type: &HashMap<&str, &str>,
    graph: &dyn GraphBackend,
    node_id: NodeId,
    errors: &mut Vec<TypeError>,
) {
    match expr {
        ValueExpr::Ref(parts) => {
            check_ref_chain(parts, scope_type, graph, node_id, errors);
        }
        ValueExpr::Old(inner) => {
            // `old(sender.balance)` — same field resolution as the non-old form.
            collect_value_field_errors(inner, scope_type, graph, node_id, errors);
        }
        ValueExpr::Arithmetic { left, right, .. } => {
            collect_value_field_errors(left, scope_type, graph, node_id, errors);
            collect_value_field_errors(right, scope_type, graph, node_id, errors);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_value_field_errors(arg, scope_type, graph, node_id, errors);
            }
        }
        ValueExpr::Set(items) => {
            for item in items {
                collect_value_field_errors(item, scope_type, graph, node_id, errors);
            }
        }
        ValueExpr::Literal(_) => {}
    }
}

/// Validate a dotted field-access chain against the type hierarchy.
///
/// `parts[0]` must be a scope-variable name; subsequent parts are field names
/// on successively nested `Describe` types.
///
/// Silently skips when:
/// - `parts` has fewer than 2 elements (single-name references like `value`
///   are not field accesses and may be quantifier variables or built-in refs).
/// - `parts[0]` is not in scope (quantifier variable — type not tracked).
///
/// Emits [`TypeError::UndefinedField`] when a field segment is not found.
fn check_ref_chain(
    parts: &[String],
    scope_type: &HashMap<&str, &str>,
    graph: &dyn GraphBackend,
    node_id: NodeId,
    errors: &mut Vec<TypeError>,
) {
    if parts.len() < 2 {
        return;
    }

    let root_name = parts[0].as_str();
    let Some(&type_ref) = scope_type.get(root_name) else {
        // Not in scope — may be a quantifier variable; skip.
        return;
    };

    resolve_field_chain(&parts[1..], type_ref, graph, node_id, errors);
}

/// Recursively walk the field chain starting at `current_type_ref`.
///
/// For each field name, find the `Describe` node for `current_type_ref` and
/// verify the field exists. If found, recurse into the field's type. If not,
/// emit [`TypeError::UndefinedField`] and stop.
fn resolve_field_chain(
    remaining_parts: &[String],
    current_type_ref: &str,
    graph: &dyn GraphBackend,
    node_id: NodeId,
    errors: &mut Vec<TypeError>,
) {
    if remaining_parts.is_empty() {
        return;
    }

    let field_name = remaining_parts[0].as_str();

    // Find the Describe node for the current type.
    let Some(type_node_id) = find_type_node_by_name(graph, current_type_ref) else {
        // The type doesn't resolve to a graph node (could be a base/builtin).
        // Base/builtin types have no named fields — accessing a field on them
        // is technically invalid, but we don't error here to keep things
        // lenient in Phase 2. Phase 3 Z3 will catch arithmetic on
        // incompatible types.
        return;
    };

    // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
    let Some(type_node) = graph.get_node(type_node_id).ok().flatten() else {
        return;
    };

    if !matches!(type_node.pattern, Pattern::Describe) {
        // Only Describe nodes have named fields.
        return;
    }

    let field = type_node
        .metadata
        .fields
        .iter()
        .find(|f| f.name == field_name);

    let Some(field) = field else {
        errors.push(TypeError::UndefinedField {
            node_id,
            type_name: current_type_ref.to_string(),
            field: field_name.to_string(),
        });
        return;
    };

    // Recurse for the remaining parts.
    resolve_field_chain(
        &remaining_parts[1..],
        &field.type_ref,
        graph,
        node_id,
        errors,
    );
}

// ─── Check 3 ─────────────────────────────────────────────────────────────────

/// Check data flow type compatibility.
///
/// For each context packet:
/// - Resolves `must_produce`, emitting [`TypeError::UndefinedType`] if unknown.
/// - Resolves each scope variable's `type_ref`, emitting [`TypeError::UndefinedType`]
///   for unknowns.
/// - If the node has a declared output type (`return_type`) **and** `must_produce`
///   is set, compares them by string equality (stripped namespace). A mismatch
///   emits [`TypeError::TypeMismatch`].
///
/// **Phase 2 limitation**: uses string equality, not structural subtyping.
/// `PositiveInteger` will not satisfy `NonNegativeInteger` even though it is
/// logically a subtype. Phase 3 Z3 handles true subtype checking.
///
/// TODO(phase-3): replace string equality with Z3-backed subtype check.
/// See AIL-Rules §5: "Same base + source constraints imply target constraints → subtype."
pub(super) fn check_data_flow_types(
    graph: &dyn GraphBackend,
    packets: &[ContextPacket],
    errors: &mut Vec<TypeError>,
) {
    for packet in packets {
        let node_id = packet.node_id;

        // Resolve every scope variable's declared type.
        for sv in &packet.scope {
            if resolve_type_ref(&sv.type_ref, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id,
                    name: sv.type_ref.clone(),
                });
            }
        }

        // Resolve must_produce.
        let must_produce_type = if let Some(mp) = &packet.must_produce {
            if resolve_type_ref(mp, graph).is_none() {
                errors.push(TypeError::UndefinedType {
                    node_id,
                    name: mp.clone(),
                });
                None
            } else {
                Some(mp.as_str())
            }
        } else {
            None
        };

        // Compare node output type vs must_produce.
        // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
        let Some(node) = graph.get_node(node_id).ok().flatten() else {
            continue;
        };
        let Some(return_type) = &node.metadata.return_type else {
            continue;
        };
        let Some(must) = must_produce_type else {
            continue;
        };

        // Normalise by stripping namespace prefix for comparison.
        let actual = return_type
            .rsplit('.')
            .next()
            .unwrap_or(return_type.as_str());
        let expected = must.rsplit('.').next().unwrap_or(must);

        if actual != expected {
            errors.push(TypeError::TypeMismatch {
                node_id,
                expected: must.to_string(),
                actual: return_type.clone(),
            });
        }
    }
}

// ─── Check 4 ─────────────────────────────────────────────────────────────────

/// Check parameter types via outgoing Ed edges to `Do` nodes.
///
/// For each node that has outgoing Ed edges to a `Do` function:
/// - Ensures every callee parameter type resolves.
/// - If the calling node's scope contains a variable with the same name as a
///   parameter but a different `type_ref`, emits [`TypeError::ParamTypeMismatch`].
///
/// **Phase 2 note**: This check fires only when Ed edges exist. Phase 4
/// (parser) creates Ed edges for function calls. Absence of Ed edges is not
/// an error — `type_check` can be re-run after parsing to catch mismatches.
pub(super) fn check_do_param_types_from_ed_edges(
    graph: &dyn GraphBackend,
    packets: &[ContextPacket],
    errors: &mut Vec<TypeError>,
) {
    let packet_by_node: HashMap<NodeId, &ContextPacket> =
        packets.iter().map(|p| (p.node_id, p)).collect();

    for node in graph.all_nodes_vec() {
        let caller_id = node.id;
        let Ok(refs) = graph.outgoing_diagonal_refs(caller_id) else {
            continue;
        };

        for callee_id in refs {
            // AilGraph: never errors; SqliteGraph: degrade gracefully on missing node.
            let Some(callee) = graph.get_node(callee_id).ok().flatten() else {
                continue;
            };
            if !matches!(callee.pattern, Pattern::Do) {
                continue;
            }

            // Build caller scope lookup. If the caller has no packet (typical
            // before Phase 4 parser creates CIC packets), the scope is empty
            // and the name-match branch below is never reached — the callee's
            // param types are still validated (resolve check), but no
            // ParamTypeMismatch can fire. This is expected Phase 2 behaviour.
            let caller_scope: HashMap<&str, &str> = packet_by_node
                .get(&caller_id)
                .map(|p| {
                    p.scope
                        .iter()
                        .map(|sv| (sv.name.as_str(), sv.type_ref.as_str()))
                        .collect()
                })
                .unwrap_or_default();

            for param in &callee.metadata.params {
                // Verify the callee parameter type resolves.
                if resolve_type_ref(&param.type_ref, graph).is_none() {
                    errors.push(TypeError::UndefinedType {
                        node_id: callee_id,
                        name: param.type_ref.clone(),
                    });
                    continue;
                }

                // If caller scope has a variable with the same name but a
                // different type, that is a mismatch.
                if let Some(&caller_type) = caller_scope.get(param.name.as_str()) {
                    let caller_norm = caller_type.rsplit('.').next().unwrap_or(caller_type);
                    let callee_norm = param.type_ref.rsplit('.').next().unwrap_or(&param.type_ref);
                    if caller_norm != callee_norm {
                        errors.push(TypeError::ParamTypeMismatch {
                            node_id: caller_id,
                            param: param.name.clone(),
                            expected: param.type_ref.clone(),
                            actual: caller_type.to_string(),
                        });
                    }
                }
            }
        }
    }
}
