//! Serialization from `VerifiedGraph` to `GraphJson` and incremental diffs.
//!
//! Public entry points:
//! - [`serialize_graph`] — full serialization of a `VerifiedGraph`.
//! - [`diff_graph`] — incremental diff between two `GraphJson` values.

pub mod diff;
pub mod externals;
pub mod graph;
pub mod issues;

pub use diff::{diff_graph, diff_graph_at};
pub use graph::{apply_verify_outcomes, serialize_graph, serialize_typed_graph, NodeVerdict};
