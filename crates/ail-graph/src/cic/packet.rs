use serde::{Deserialize, Serialize};

use crate::types::NodeId;

use super::{coverage_info::CoverageInfo, PacketConstraint, PromotedFact, ScopeVariable};

/// The cumulative inherited context available at a single node.
///
/// A packet is computed deterministically by
/// [`crate::AilGraph::compute_context_packet`] and contains everything a
/// downstream phase needs to reason about the node in isolation:
///
/// - What ancestor obligations flow in (Rule 1 DOWN).
/// - Which type constraints auto-inject because of typed scope variables
///   (Rule 4 DIAGONAL, resolved from [`ContextPacket::scope`]).
/// - Which call contracts flow in from explicit Ed function references
///   (Rule 4 DIAGONAL, resolved from outgoing Ed edges).
/// - Which variables are in scope (Rule 1 DOWN + Rule 3 ACROSS, depth-aware
///   across all ancestor levels).
/// - The return type the node is expected to produce (from the nearest
///   enclosing `Do`).
///
/// Two fields stay empty in Phase 1: [`ContextPacket::verified_facts`]
/// (Rule 2 UP requires Phase 3 Z3 verification) and
/// [`ContextPacket::template_constraints`] (the `Following`/`Using` pattern
/// variants do not yet exist in the pattern enum).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPacket {
    /// The node the packet was computed for.
    pub node_id: NodeId,
    /// Root-to-current intent path (inclusive on both ends).
    pub intent_chain: Vec<String>,
    /// Constraints inherited from every ancestor via Ev edges (Rule 1 DOWN).
    /// Ordered root-first.
    pub inherited_constraints: Vec<PacketConstraint>,
    /// Constraints auto-injected from typed scope variables (Rule 4 DIAGONAL,
    /// type branch). Unfolded recursively through record fields.
    pub type_constraints: Vec<PacketConstraint>,
    /// Pre/post contracts of functions explicitly called via outgoing Ed
    /// edges from the node or any ancestor (Rule 4 DIAGONAL, call branch).
    pub call_contracts: Vec<PacketConstraint>,
    /// Constraints injected by a `following`/`using` template reference
    /// (Rule 4 DIAGONAL, template branch).
    ///
    /// Empty in Phase 1: the pattern enum does not yet include `Following`
    /// or `Using` variants. Populated once those patterns ship.
    pub template_constraints: Vec<PacketConstraint>,
    /// Postconditions promoted up from verified sibling/child nodes
    /// (Rule 2 UP + verified half of Rule 3 ACROSS).
    ///
    /// Empty in Phase 1: promotion requires Phase 3 Z3 verification results.
    pub verified_facts: Vec<PacketConstraint>,
    /// Facts proved by preceding `check X otherwise raise E` nodes (Phase 8).
    ///
    /// Collected from Check siblings before this node at every ancestor level
    /// (depth-aware, same walk as scope assembly). Also includes checks inside
    /// preceding sibling `Do` bodies (Rule P2 UP). Empty when no check nodes
    /// precede this node in its execution path.
    ///
    /// `#[serde(default)]` ensures cached packets serialised before Phase 8
    /// deserialise without error — they simply get an empty `promoted_facts`.
    #[serde(default)]
    pub promoted_facts: Vec<PromotedFact>,
    /// All named variables available at the current node.
    pub scope: Vec<ScopeVariable>,
    /// Return-type text from the nearest enclosing `Do`, if any.
    pub must_produce: Option<String>,
    /// Semantic coverage result for this node, computed by `ail-coverage`.
    ///
    /// `None` until coverage is explicitly computed. `#[serde(default)]`
    /// ensures packets serialised before coverage support deserialise without
    /// error — they simply get `coverage: None`.
    #[serde(default)]
    pub coverage: Option<CoverageInfo>,
}

impl ContextPacket {
    /// Create an empty packet anchored at `node_id`. Used as a starting
    /// point by [`crate::AilGraph::compute_context_packet`].
    pub(crate) fn empty_for(node_id: NodeId) -> Self {
        Self {
            node_id,
            intent_chain: Vec::new(),
            inherited_constraints: Vec::new(),
            type_constraints: Vec::new(),
            call_contracts: Vec::new(),
            template_constraints: Vec::new(),
            verified_facts: Vec::new(),
            promoted_facts: Vec::new(),
            scope: Vec::new(),
            must_produce: None,
            coverage: None,
        }
    }
}
