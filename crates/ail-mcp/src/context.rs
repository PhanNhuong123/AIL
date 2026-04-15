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
}
