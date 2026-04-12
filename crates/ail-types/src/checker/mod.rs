mod checks;
mod resolver;

use ail_graph::{ContextPacket, ValidGraph};

use crate::errors::TypeError;
use crate::types::TypedGraph;

use checks::{
    check_all_node_type_refs, check_contract_field_access, check_data_flow_types,
    check_do_param_types_from_ed_edges,
};

/// Run all type checks on a [`ValidGraph`] and produce a [`TypedGraph`].
///
/// Checks performed (in order):
/// 1. **Type-reference resolution** — every `type_ref` string in node metadata
///    must name a known base type, builtin semantic type, or user-defined graph
///    node. Emits [`TypeError::UndefinedType`].
/// 2. **Contract field access** — field-access chains in contract expressions
///    (e.g. `sender.balance`) must resolve against the scope and type hierarchy.
///    Emits [`TypeError::UndefinedField`].
/// 3. **Data flow types** — scope variable types and `must_produce` types must
///    resolve; when both a node output type and `must_produce` are present they
///    must match (string equality in Phase 2). Emits [`TypeError::UndefinedType`]
///    and [`TypeError::TypeMismatch`].
/// 4. **Parameter types via Ed edges** — when a node calls a `Do` function via
///    an outgoing Ed edge, the caller's scope variable types must match the
///    callee's declared parameter types. Emits [`TypeError::ParamTypeMismatch`].
///
/// All errors are accumulated in a single pass. Returns `Err(errors)` if any
/// check failed, or `Ok(TypedGraph)` when the graph is clean.
pub fn type_check(
    valid: ValidGraph,
    packets: &[ContextPacket],
) -> Result<TypedGraph, Vec<TypeError>> {
    let mut errors = Vec::new();

    check_all_node_type_refs(valid.graph(), &mut errors);
    check_contract_field_access(valid.graph(), packets, &mut errors);
    check_data_flow_types(valid.graph(), packets, &mut errors);
    check_do_param_types_from_ed_edges(valid.graph(), packets, &mut errors);

    if errors.is_empty() {
        Ok(TypedGraph::new(valid))
    } else {
        Err(errors)
    }
}
