use std::fmt;

use ail_graph::{AilGraph, GraphBackend, ValidGraph};

/// A graph that has passed all type-checking rules.
///
/// `TypedGraph` is the second hard stage gate in the AIL pipeline. It wraps a
/// [`ValidGraph`] and certifies that every type reference resolves, all field
/// accesses in contract expressions are valid, data flow types are compatible,
/// and function call parameter types match.
///
/// The only way to obtain a `TypedGraph` is through [`crate::type_check`].
/// Downstream crates (`ail-contract`) require `TypedGraph` as input.
#[derive(Clone)]
pub struct TypedGraph(ValidGraph);

impl TypedGraph {
    /// Wrap a validated graph that has passed all type checks.
    /// Only callable within this crate.
    pub(crate) fn new(valid: ValidGraph) -> Self {
        Self(valid)
    }

    /// Return a shared reference to the underlying graph as a [`GraphBackend`] trait object.
    ///
    /// Downstream pipeline stages accept `&dyn GraphBackend` so they remain
    /// backend-agnostic. Use [`into_inner`] when you need the concrete type.
    ///
    /// [`into_inner`]: TypedGraph::into_inner
    pub fn graph(&self) -> &dyn GraphBackend {
        self.0.graph()
    }

    /// Return a shared reference to the underlying [`AilGraph`].
    ///
    /// For use by tools (MCP, CLI) that need `AilGraph`-specific APIs such as
    /// BM25 search or edge count. Prefer [`graph`] for backend-agnostic pipeline code.
    ///
    /// [`graph`]: TypedGraph::graph
    pub fn ail_graph(&self) -> &AilGraph {
        self.0.ail_graph()
    }

    /// Consume the `TypedGraph` and return the inner `ValidGraph`.
    ///
    /// Used by the next pipeline stage (`ail-contract`) to take ownership.
    pub fn into_inner(self) -> ValidGraph {
        self.0
    }
}

impl fmt::Debug for TypedGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedGraph")
            .field("node_count", &self.0.graph().node_count())
            .finish()
    }
}
