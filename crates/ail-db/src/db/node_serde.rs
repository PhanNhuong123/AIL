use ail_graph::types::{Contract, ContractKind, Expression, Node, NodeId, NodeMetadata, Pattern};

use crate::errors::DbError;

// ─── Pattern ─────────────────────────────────────────────────────────────────

/// Serialize a `Pattern` to its snake_case SQL string (e.g. `"do"`, `"for_each"`).
pub(crate) fn pattern_to_sql(pattern: &Pattern) -> Result<String, DbError> {
    match serde_json::to_value(pattern)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(DbError::Other(format!(
            "pattern serialized to unexpected JSON value: {other}"
        ))),
    }
}

/// Deserialize a snake_case SQL string back to a `Pattern`.
pub(crate) fn pattern_from_sql(s: &str) -> Result<Pattern, DbError> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(DbError::Serialization)
}

// ─── ContractKind ─────────────────────────────────────────────────────────────

/// Serialize a `ContractKind` to its snake_case SQL string.
pub(crate) fn contract_kind_to_sql(kind: &ContractKind) -> Result<String, DbError> {
    match serde_json::to_value(kind)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(DbError::Other(format!(
            "contract kind serialized to unexpected JSON value: {other}"
        ))),
    }
}

/// Deserialize a snake_case SQL string back to a `ContractKind`.
pub(crate) fn contract_kind_from_sql(s: &str) -> Result<ContractKind, DbError> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(DbError::Serialization)
}

// ─── NodeId ───────────────────────────────────────────────────────────────────

/// Parse a UUID TEXT column value from the database into a `NodeId`.
pub(crate) fn node_id_from_sql(s: &str) -> Result<NodeId, DbError> {
    s.parse::<NodeId>()
        .map_err(|e| DbError::InvalidUuid(format!("{s}: {e}")))
}

// ─── Node → SQL params ───────────────────────────────────────────────────────

/// Intermediate representation used to pass node fields as SQL parameters.
pub(crate) struct NodeRow {
    pub id: String,
    pub intent: String,
    pub name: Option<String>,
    pub pattern: String,
    pub expression: Option<String>,
    pub metadata: String,
}

/// Convert a `Node` to a `NodeRow` ready for SQL insertion.
///
/// Does NOT include `parent_id`, `position`, or `depth` — those are set by
/// edge operations, not by the node content itself.
pub(crate) fn node_to_row(node: &Node) -> Result<NodeRow, DbError> {
    Ok(NodeRow {
        id: node.id.to_string(),
        intent: node.intent.clone(),
        name: node.metadata.name.clone(),
        pattern: pattern_to_sql(&node.pattern)?,
        expression: node.expression.as_ref().map(|e| e.0.clone()),
        metadata: serde_json::to_string(&node.metadata)?,
    })
}

// ─── SQL row → Node ──────────────────────────────────────────────────────────

/// Reconstruct a `Node` from individual SQL column values.
///
/// `contracts` must be loaded separately and passed in.
/// `children` is always `None` — callers use `GraphBackend::children()` for that.
pub(crate) fn row_to_node(
    id_str: String,
    intent: String,
    pattern_str: String,
    expression: Option<String>,
    metadata_str: String,
    contracts: Vec<Contract>,
) -> Result<Node, DbError> {
    let id = node_id_from_sql(&id_str)?;
    let pattern = pattern_from_sql(&pattern_str)?;
    let metadata: NodeMetadata = serde_json::from_str(&metadata_str)?;
    let expression = expression.map(Expression);

    Ok(Node {
        id,
        intent,
        pattern,
        children: None,
        expression,
        contracts,
        metadata,
    })
}
