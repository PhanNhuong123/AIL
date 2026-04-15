mod after_contract;
mod before_contract;
mod following_phases;
mod raise_error;
pub mod scope;

use ail_graph::types::Pattern;
use ail_types::TypedGraph;

use crate::errors::ContractError;

use after_contract::check_after_contracts;
use before_contract::check_before_contracts;
use following_phases::check_following_template_phases;
use raise_error::check_raise_error_refs;

/// Run all Phase 3.1 static contract checks over a [`TypedGraph`].
///
/// Returns every contract error found. An empty `Vec` means all contracts
/// passed the static checks. Errors from one node do not stop checks on
/// other nodes — the full list is always accumulated.
///
/// Checks performed:
/// - **Before-contract scope** (AIL-C001, C002): no `old()`, only input params.
/// - **After-contract scope** (AIL-C003): only params + `"result"` as direct refs.
/// - **Raise error refs** (AIL-C004): raised error must be declared by enclosing `Do`.
/// - **Template phase coverage** (AIL-C005): implementing `Do` covers all template phases.
/// - **Parse errors** (AIL-C006): contract expression must be parseable.
pub fn check_static_contracts(typed_graph: &TypedGraph) -> Vec<ContractError> {
    let graph = typed_graph.graph();
    let mut errors = Vec::new();

    for node in graph.all_nodes_vec() {
        check_before_contracts(graph, &node, &mut errors);
        check_after_contracts(graph, &node, &mut errors);

        if node.pattern == Pattern::Raise {
            check_raise_error_refs(graph, &node, &mut errors);
        }

        if node.pattern == Pattern::Do {
            check_following_template_phases(graph, &node, &mut errors);
        }
    }

    errors
}
