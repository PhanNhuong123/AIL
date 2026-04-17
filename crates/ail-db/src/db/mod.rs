mod backend_impl;
pub(crate) mod cic_cache;
pub(crate) mod coverage;
mod embedding;
pub(crate) mod fts_search;
mod node_serde;
mod schema;
pub mod sqlite_graph;

pub use embedding::EmbeddingModelStatus;
pub use sqlite_graph::SqliteGraph;
