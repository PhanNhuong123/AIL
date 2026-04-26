use ail_graph::NodeId;
use ail_types::ConstraintExpr;
use serde::{Deserialize, Serialize};

/// Local constraint section for one `Do` node.
///
/// `constraints` — node-local Before/After/Always contracts parsed into
/// `ConstraintExpr`. `inherited` — CIC `inherited_constraints` + `promoted_facts`,
/// each AND-split at compound boundaries. Only `Do` nodes receive sections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SheafSection {
    pub node_id: NodeId,
    pub constraints: Vec<ConstraintExpr>,
    pub inherited: Vec<ConstraintExpr>,
}

/// Pair of related sections with their combined constraint view.
///
/// Pair ordering is **execution-order**, not lexical: for parent-child,
/// `node_a` = parent. For siblings, `node_a` = the earlier-in-Eh-chain sibling
/// (the one returned by `siblings_before`). Both nodes are retained so that
/// Phase 17.2 can attribute conflicts to either side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SheafOverlap {
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub combined: Vec<ConstraintExpr>,
}

/// The Čech nerve over a `VerifiedGraph`.
///
/// `sections` is sorted by `node_id.to_string()`.
/// `overlaps` is sorted by `(node_a.to_string(), node_b.to_string())`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CechNerve {
    pub sections: Vec<SheafSection>,
    pub overlaps: Vec<SheafOverlap>,
}
