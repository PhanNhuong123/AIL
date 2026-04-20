use serde::{Deserialize, Serialize};

use super::status::Status;

/// A complete flowchart for a single function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowchartJson {
    pub nodes: Vec<FlowNodeJson>,
    pub edges: Vec<FlowEdgeJson>,
}

/// A node within a flowchart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowNodeJson {
    pub id: String,
    pub kind: FlowNodeKind,
    pub label: String,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
}

/// Visual shape kind for a flowchart node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlowNodeKind {
    Start,
    End,
    Process,
    Decision,
    Io,
    Sub,
    Storage,
}

/// A directed edge within a flowchart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowEdgeJson {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}
