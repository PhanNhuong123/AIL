//! `VerifiedGraph` — the third hard pipeline stage gate.
//!
//! A `VerifiedGraph` certifies that the wrapped [`TypedGraph`] has passed all
//! static contract scope checks and (when the `z3-verify` feature is enabled)
//! all Z3 contract verification checks. It is the only output accepted by the
//! emit stage (`ail-emit`).
//!
//! The only way to obtain a `VerifiedGraph` is through [`crate::verify`].

use std::fmt;
use std::sync::OnceLock;

use ail_graph::AilGraph;
use ail_types::TypedGraph;

use super::ContractSummary;

/// A graph that has passed all static contract checks and Z3 verification.
///
/// `VerifiedGraph` is the third hard stage gate in the AIL pipeline:
/// `ValidGraph → TypedGraph → VerifiedGraph`.
///
/// Downstream crates (`ail-emit`) require `VerifiedGraph` as input and cannot
/// accept a raw `TypedGraph` or `AilGraph`.
///
/// The only way to obtain a `VerifiedGraph` is through [`crate::verify`].
pub struct VerifiedGraph {
    typed: TypedGraph,
    /// Lazily computed contract summary. Populated on first call to
    /// [`contract_summary`]. `OnceLock` ensures at-most-one computation even if
    /// called multiple times.
    ///
    /// [`contract_summary`]: VerifiedGraph::contract_summary
    summary: OnceLock<ContractSummary>,
}

impl VerifiedGraph {
    /// Wrap a typed graph that has passed all contract checks.
    /// Only callable within this crate.
    pub(crate) fn new(typed: TypedGraph) -> Self {
        Self {
            typed,
            summary: OnceLock::new(),
        }
    }

    /// Return a shared reference to the underlying graph.
    pub fn graph(&self) -> &AilGraph {
        self.typed.graph()
    }

    /// Return a shared reference to the inner [`TypedGraph`].
    ///
    /// Useful when a downstream tool needs access to type-level metadata
    /// without consuming the `VerifiedGraph`.
    pub fn typed(&self) -> &TypedGraph {
        &self.typed
    }

    /// Return the contract summary for this verified graph, computing and
    /// caching it on the first call.
    ///
    /// The summary captures every `Do` node's before/after/always contract
    /// expressions and is used by CLI tools for breaking-change detection
    /// (`ail build --check-breaking`).
    ///
    /// # Note
    /// Computation is O(nodes). With `OnceLock`, repeated calls return the
    /// cached result without re-walking the graph.
    pub fn contract_summary(&self) -> &ContractSummary {
        self.summary
            .get_or_init(|| ContractSummary::from_graph(self.graph()))
    }

    /// Consume the `VerifiedGraph` and return the inner [`TypedGraph`].
    ///
    /// Used by the next pipeline stage (`ail-emit`) to take ownership.
    pub fn into_inner(self) -> TypedGraph {
        self.typed
    }
}

impl fmt::Debug for VerifiedGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedGraph")
            .field("node_count", &self.graph().node_count())
            .finish()
    }
}
