use std::fmt;

use crate::graph::{AilGraph, GraphBackend};

/// A graph that has passed all structural and semantic validation rules.
///
/// `ValidGraph` is the first hard stage gate in the AIL pipeline. Downstream
/// crates (`ail-types`, `ail-contract`, `ail-emit`) require `ValidGraph` as
/// input and cannot accept a raw `AilGraph`.
///
/// The only way to obtain a `ValidGraph` is through [`crate::validate_graph`].
#[derive(Clone)]
pub struct ValidGraph(AilGraph);

impl ValidGraph {
    /// Wrap a validated graph. Only callable within this crate.
    pub(crate) fn new(graph: AilGraph) -> Self {
        Self(graph)
    }

    /// Return a shared reference to the underlying graph as a [`GraphBackend`] trait object.
    ///
    /// Downstream pipeline stages accept `&dyn GraphBackend` so they remain
    /// backend-agnostic. Use [`into_inner`] when you need the concrete `AilGraph`.
    ///
    /// [`into_inner`]: ValidGraph::into_inner
    pub fn graph(&self) -> &dyn GraphBackend {
        &self.0
    }

    /// Return a shared reference to the underlying [`AilGraph`].
    ///
    /// For use by tools (MCP, CLI) that need `AilGraph`-specific APIs such as
    /// BM25 search or edge count. Prefer [`graph`] for backend-agnostic pipeline code.
    ///
    /// [`graph`]: ValidGraph::graph
    pub fn ail_graph(&self) -> &AilGraph {
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
