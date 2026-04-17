//! `ail-graph` — PSSD graph, CIC packets, BM25 search, validation, and `ValidGraph`.
//!
//! This crate is the first stage of the AIL compiler pipeline. It owns:
//! - The core graph data model ([`AilGraph`], [`Node`], [`EdgeKind`]).
//! - CIC (Constraint Inheritance Chain) computation ([`ContextPacket`], [`PacketConstraint`]).
//! - BM25 full-text search over the graph ([`Bm25Index`]).
//! - Folder-index generation and name resolution ([`FolderIndex`], [`NameResolver`]).
//! - Structural validation producing the first hard pipeline gate ([`ValidGraph`]).
//!
//! ## Pipeline position
//!
//! ```text
//! AilGraph → validate_graph() → ValidGraph → (ail-types)
//! ```
//!
//! ## Entry points
//!
//! - [`AilGraphBuilder`] — build a graph from parsed nodes and edges.
//! - [`validate_graph`] — validate a raw graph; returns [`ValidGraph`] or errors.
//! - [`ContextPacket`] — query CIC-propagated constraints for any node.

pub mod cic;
pub mod constants;
pub mod errors;
pub mod graph;
pub mod index;
pub mod search;
pub mod types;
pub mod validation;

pub use cic::{
    check_promotion_affected_nodes, compute_context_packet_for_backend, ChildContributionInfo,
    ContextPacket, CoverageConfig, CoverageInfo, CoverageStatus, FactOrigin, MissingAspectInfo,
    PacketConstraint, PromotedFact, ScopeVariable, ScopeVariableKind,
};
pub use errors::{GraphError, ValidationError};
pub use graph::{AilGraph, AilGraphBuilder, GraphBackend};
pub use index::{
    generate_folder_index_for_node, render_folder_index, ContractSummary, FolderIndex, IndexEntry,
    IndexKind, NameResolver,
};
pub use search::{Bm25Index, SearchResult};
pub use types::{
    Contract, ContractKind, EdgeId, EdgeKind, Expression, Field, Node, NodeId, NodeMetadata, Param,
    Pattern,
};
pub use validation::{validate_graph, ValidGraph};
