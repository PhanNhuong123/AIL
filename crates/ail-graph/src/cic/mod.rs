//! Cumulative Inherited Context (CIC) engine.
//!
//! Computes a deterministic [`ContextPacket`] for any node by propagating
//! information along the graph's three edge types:
//!
//! - Rule 1 DOWN — ancestor contracts flow into `inherited_constraints`.
//! - Rule 2 UP — verified child postconditions become parent facts
//!   (structurally prepared here; populated by Phase 3 `ail-contract`).
//! - Rule 3 ACROSS — previous sibling outputs flow into the next sibling's
//!   scope, depth-aware across all ancestor levels so nested nodes can see
//!   uncle let-bindings.
//! - Rule 4 DIAGONAL — type constraints auto-inject from typed scope
//!   variables, and call contracts flow in from outgoing Ed edges.
//!
//! The engine stays AST-agnostic — constraints are carried as raw
//! [`crate::types::Expression`] text plus origin [`crate::types::NodeId`].
//! Evaluation and Z3 encoding are Phase 2 / 3 concerns.

mod compute;
mod compute_backend;
mod constraint;
mod invalidation;
mod packet;
mod promotion;
mod scope;

pub use compute_backend::compute_context_packet_for_backend;
pub use constraint::PacketConstraint;
pub use invalidation::check_promotion_affected_nodes;
pub use packet::ContextPacket;
pub use promotion::{FactOrigin, PromotedFact};
pub use scope::{ScopeVariable, ScopeVariableKind};
