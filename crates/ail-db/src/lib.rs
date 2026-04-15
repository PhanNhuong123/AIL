//! `ail-db` — SQLite-backed graph storage for the AIL pipeline.
//!
//! Provides [`SqliteGraph`], which implements the [`ail_graph::GraphBackend`]
//! trait backed by a single `.ail.db` SQLite file. WAL mode and foreign-key
//! constraints are enabled on every connection open.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use std::path::Path;
//! use ail_db::SqliteGraph;
//! use ail_graph::{GraphBackend, Node, NodeId, Pattern};
//!
//! let mut db = SqliteGraph::create(Path::new("project.ail.db")).unwrap();
//! let node = Node::new(NodeId::new(), "validate input", Pattern::Do);
//! let id = db.add_node(node).unwrap();
//! ```

mod db;
pub mod errors;

pub use db::SqliteGraph;
pub use errors::DbError;
