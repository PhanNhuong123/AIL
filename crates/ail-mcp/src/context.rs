//! Pipeline state holder for the MCP server.
//!
//! [`ProjectContext`] wraps the highest-available pipeline stage so tool
//! handlers can access the underlying [`AilGraph`] and stage-specific APIs
//! through a single unified type.

use ail_contract::VerifiedGraph;
use ail_graph::{AilGraph, ValidGraph};
use ail_types::TypedGraph;

/// The pipeline stage reached for the current project.
///
/// Each variant wraps the completed stage gate. Because the gates are nested
/// (each consumes the prior), this enum holds only the highest level and
/// exposes downward accessors.
pub enum ProjectContext {
    /// Graph built in memory but not yet validated.
    Raw(AilGraph),
    /// Passed `validate_graph`.
    Valid(ValidGraph),
    /// Passed `type_check`.
    Typed(TypedGraph),
    /// Passed `verify` — all pipeline stages complete.
    Verified(VerifiedGraph),
}

impl ProjectContext {
    /// The underlying raw graph, available at every stage.
    pub fn graph(&self) -> &AilGraph {
        match self {
            Self::Raw(g) => g,
            Self::Valid(v) => v.ail_graph(),
            Self::Typed(t) => t.ail_graph(),
            Self::Verified(v) => v.ail_graph(),
        }
    }

    /// Human-readable name for the current stage.
    pub fn stage_name(&self) -> &str {
        match self {
            Self::Raw(_) => "raw",
            Self::Valid(_) => "validated",
            Self::Typed(_) => "typed",
            Self::Verified(_) => "verified",
        }
    }

    /// Returns a reference to the `VerifiedGraph` if this is the `Verified` stage.
    pub fn as_verified(&self) -> Option<&VerifiedGraph> {
        match self {
            Self::Verified(v) => Some(v),
            _ => None,
        }
    }

    /// Returns a reference to the `TypedGraph` if the context is `Typed` or `Verified`.
    pub fn as_typed(&self) -> Option<&TypedGraph> {
        match self {
            Self::Typed(t) => Some(t),
            Self::Verified(v) => Some(v.typed()),
            _ => None,
        }
    }

    /// Return a mutable reference to the underlying `AilGraph`, demoting the
    /// pipeline stage to `Raw`.
    ///
    /// Write operations invalidate validation, type-checking, and verification
    /// guarantees, so the context drops back to `Raw` on first mutable access.
    /// Callers should clear search caches after calling this.
    pub fn graph_mut(&mut self) -> &mut AilGraph {
        if !matches!(self, Self::Raw(_)) {
            let taken = std::mem::replace(self, ProjectContext::Raw(AilGraph::new()));
            let graph = match taken {
                ProjectContext::Raw(g) => g,
                ProjectContext::Valid(v) => v.into_inner(),
                ProjectContext::Typed(t) => t.into_inner().into_inner(),
                ProjectContext::Verified(v) => v.into_inner().into_inner().into_inner(),
            };
            *self = ProjectContext::Raw(graph);
        }
        match self {
            Self::Raw(g) => g,
            _ => unreachable!(),
        }
    }
}
