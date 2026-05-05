use serde::{Deserialize, Serialize};

use super::status::Status;

/// Detailed information about a single node, keyed by path ID in `GraphJson.detail`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDetail {
    pub name: String,
    pub status: Status,
    pub description: String,
    pub receives: Vec<ReceivesEntry>,
    pub returns: Vec<ReturnsEntry>,
    pub rules: Vec<RuleEntry>,
    pub inherited: Vec<InheritedRule>,
    pub proven: Vec<String>,
    pub verification: VerificationDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<CodeBlob>,
}

/// A parameter or input to the node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceivesEntry {
    pub name: String,
    pub desc: String,
}

/// A return value or output from the node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnsEntry {
    pub name: String,
    pub desc: String,
}

/// A rule (contract expression) attached to or inherited by a node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleEntry {
    pub text: String,
    pub source: RuleSource,
}

/// Whether a rule originates from the node itself or is inherited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSource {
    Own,
    Inherited,
}

/// A rule inherited from an ancestor node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InheritedRule {
    pub text: String,
    pub from: String,
}

/// Per-node Z3 solver verdict surfaced in the IDE verify pill.
///
/// Naming follows the user-facing convention from the v4.0 ship checklist:
/// `Sat` = postcondition entailed (✓ Verified); `Unsat` = counterexample
/// found; `Unknown` = solver gave up; `Timeout` = 30s limit hit. Mirrors the
/// frontend `VerifyOutcome` union in `ide/src/lib/types.ts` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerifyOutcome {
    Sat,
    Unsat,
    Unknown,
    Timeout,
}

/// Formal verification result for this node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationDetail {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<CounterexampleDetail>,
    /// Z3 solver verdict for this node. `None` when verification did not run
    /// (typed-only path) or when the node is structural (not a `Do` node with
    /// contracts). Frontend renders the verify pill from `(ok, outcome)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<VerifyOutcome>,
}

/// A counterexample produced by the Z3 solver when verification fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterexampleDetail {
    pub scenario: String,
    pub effect: String,
    pub violates: String,
}

/// Generated code for a node (Python and/or TypeScript).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeBlob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typescript: Option<String>,
}
