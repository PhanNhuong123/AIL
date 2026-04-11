use serde::{Deserialize, Serialize};

use crate::types::{ContractKind, Field, NodeId, Param};

/// A condensed contract on a function signature for use in `.index.ail` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSummary {
    /// When the obligation applies.
    pub kind: ContractKind,
    /// Raw promise expression text, e.g. `"sender.balance >= amount"`.
    pub expression: String,
}

/// The declaration kind stored in a folder index.
///
/// Only four AIL patterns produce indexable declarations:
/// `Define`, `Describe` → `Type`; `Error` → `ErrorType`; `Do` → `Function`.
/// All other patterns (Let, Check, Promise, …) are implementation details and are excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexKind {
    /// `define Name:BaseType where constraint` — semantic type alias or record.
    ///
    /// `constraint_expr` is sourced from the node's `expression` field (raw text).
    /// `fields` is non-empty for `Describe` nodes; empty for `Define` nodes.
    Type {
        base_type: Option<String>,
        constraint_expr: Option<String>,
        fields: Vec<Field>,
    },

    /// `error Name carries field:Type, ...` — error payload type.
    ErrorType { carries: Vec<Field> },

    /// `do name from param:Type, ... -> ReturnType` plus contracts.
    Function {
        params: Vec<Param>,
        return_type: Option<String>,
        contracts: Vec<ContractSummary>,
    },
}

/// A single named declaration extracted from the graph for inclusion in a folder index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Symbol name (from `node.metadata.name`).
    pub name: String,
    /// What kind of declaration this is.
    pub kind: IndexKind,
    /// The graph node this entry was extracted from.
    pub node_id: NodeId,
}
