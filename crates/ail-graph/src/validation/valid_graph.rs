use std::fmt;

use crate::graph::AilGraph;

/// A graph that has passed all structural and semantic validation rules.
///
/// `ValidGraph` is the first hard stage gate in the AIL pipeline. Downstream
/// crates (`ail-types`, `ail-contract`, `ail-emit`) require `ValidGraph` as
/// input and cannot accept a raw `AilGraph`.
///
/// The only way to obtain a `ValidGraph` is through [`crate::validate_graph`].
pub struct ValidGraph(AilGraph);

impl ValidGraph {
    /// Wrap a validated graph. Only callable within this crate.
    pub(crate) fn new(graph: AilGraph) -> Self {
        Self(graph)
    }

    /// Return a shared reference to the underlying graph.
    pub fn graph(&self) -> &AilGraph {
        &self.0
    }

    /// Consume the `ValidGraph` and return the inner `AilGraph`.
    ///
    /// Used by the next pipeline stage (`ail-types`) to take ownership.
    pub fn into_inner(self) -> AilGraph {
        self.0
    }
}

impl fmt::Debug for ValidGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidGraph")
            .field("node_count", &self.0.node_count())
            .finish()
    }
}
