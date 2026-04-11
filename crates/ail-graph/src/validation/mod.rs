mod rules;
mod valid_graph;

pub use valid_graph::ValidGraph;

use crate::errors::ValidationError;
use crate::graph::AilGraph;

/// Validate `graph` against all structural and semantic rules.
///
/// On success returns a [`ValidGraph`] — the first hard stage gate in the AIL
/// pipeline.  On failure returns **all** errors found (not just the first one)
/// so the caller can see the full picture in one pass.
pub fn validate_graph(graph: AilGraph) -> Result<ValidGraph, Vec<ValidationError>> {
    let errors = rules::run_all_rules(&graph);
    if errors.is_empty() {
        Ok(ValidGraph::new(graph))
    } else {
        Err(errors)
    }
}
