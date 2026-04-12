use std::fmt;

use ail_graph::{AilGraph, ValidGraph};

/// A graph that has passed all type-checking rules.
///
/// `TypedGraph` is the second hard stage gate in the AIL pipeline. It wraps a
/// [`ValidGraph`] and certifies that every type reference resolves, all field
/// accesses in contract expressions are valid, data flow types are compatible,
/// and function call parameter types match.
///
/// The only way to obtain a `TypedGraph` is through [`crate::type_check`].
/// Downstream crates (`ail-contract`) require `TypedGraph` as input.
pub struct TypedGraph(ValidGraph);

impl TypedGraph {
    /// Wrap a validated graph that has passed all type checks.
    /// Only callable within this crate.
    pub(crate) fn new(valid: ValidGraph) -> Self {
        Self(valid)
    }

    /// Return a shared reference to the underlying graph.
    pub fn graph(&self) -> &AilGraph {
        self.0.graph()
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
