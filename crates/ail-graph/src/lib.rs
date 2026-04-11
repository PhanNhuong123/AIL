pub mod cic;
pub mod errors;
pub mod graph;
pub mod types;

pub use cic::{ContextPacket, PacketConstraint, ScopeVariable, ScopeVariableKind};
pub use errors::GraphError;
pub use graph::{AilGraph, AilGraphBuilder};
pub use types::{
    Contract, ContractKind, EdgeId, EdgeKind, Expression, Field, Node, NodeId, NodeMetadata, Param,
    Pattern,
};
