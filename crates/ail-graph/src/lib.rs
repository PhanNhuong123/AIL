pub mod cic;
pub mod errors;
pub mod graph;
pub mod index;
pub mod search;
pub mod types;

pub use cic::{ContextPacket, PacketConstraint, ScopeVariable, ScopeVariableKind};
pub use errors::GraphError;
pub use graph::{AilGraph, AilGraphBuilder};
pub use index::{
    generate_folder_index_for_node, render_folder_index, ContractSummary, FolderIndex,
    IndexEntry, IndexKind, NameResolver,
};
pub use search::{Bm25Index, SearchResult};
pub use types::{
    Contract, ContractKind, EdgeId, EdgeKind, Expression, Field, Node, NodeId, NodeMetadata, Param,
    Pattern,
};
