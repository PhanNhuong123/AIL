//! Contract summary for breaking-change detection in later CLI workflows.
//!
//! [`ContractSummary`] captures a snapshot of every `Do` node's contract
//! expressions. CLI tools compare two summaries to detect breaking changes
//! (e.g. `ail build --check-breaking`).
//!
//! # Limitation (v0.1)
//!
//! Comparison uses **raw string equality** on contract expressions.
//! Semantically equivalent expressions that differ only in whitespace or
//! formatting will be flagged as changes. Phase 4 parser normalization will
//! improve accuracy.

use std::collections::BTreeMap;

use ail_graph::types::{ContractKind, Pattern};
use ail_graph::{AilGraph, NodeId};

// ── Public types ──────────────────────────────────────────────────────────────

/// Snapshot of all `Do`-node contracts in a [`VerifiedGraph`].
///
/// Keyed by function name (falling back to `intent` when `metadata.name` is
/// absent). Uses [`BTreeMap`] for deterministic ordering so diffs and
/// serialized forms are reproducible.
///
/// [`VerifiedGraph`]: crate::VerifiedGraph
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractSummary {
    /// Contract records keyed by function name.
    pub entries: BTreeMap<String, ContractRecord>,
}

/// The contract expressions attached to one `Do` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractRecord {
    /// Graph id of the `Do` node this record was built from.
    pub node_id: NodeId,
    /// Raw text of every `Before` contract expression, in declaration order.
    pub before: Vec<String>,
    /// Raw text of every `After` contract expression, in declaration order.
    pub after: Vec<String>,
    /// Raw text of every `Always` contract expression, in declaration order.
    pub always: Vec<String>,
}

/// A single breaking change detected by [`ContractSummary::breaking_changes`].
///
/// A change is considered breaking when a `Do` function is removed from the
/// verified graph, or when a contract expression is added or removed (which
/// tightens or relaxes pre/post conditions visible to callers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakingChange {
    /// A `Do` function present in the baseline no longer exists.
    FunctionRemoved { name: String },
    /// A contract was added that was not in the baseline.
    ContractAdded {
        function: String,
        /// `"before"`, `"after"`, or `"always"`.
        kind: String,
        expr: String,
    },
    /// A contract present in the baseline was removed.
    ContractRemoved {
        function: String,
        /// `"before"`, `"after"`, or `"always"`.
        kind: String,
        expr: String,
    },
}

// ── ContractSummary impl ──────────────────────────────────────────────────────

impl ContractSummary {
    /// Build a `ContractSummary` by walking all `Do` nodes in `graph`.
    ///
    /// Nodes are keyed by `metadata.name` when present, falling back to
    /// `intent`. When two `Do` nodes share the same key, the later one in
    /// iteration order wins (graph iteration order is stable but unspecified).
    pub fn from_graph(graph: &AilGraph) -> Self {
        let mut entries: BTreeMap<String, ContractRecord> = BTreeMap::new();

        for node in graph.all_nodes() {
            if node.pattern != Pattern::Do {
                continue;
            }

            let key = node
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| node.intent.clone());

            let mut record = ContractRecord {
                node_id: node.id,
                before: Vec::new(),
                after: Vec::new(),
                always: Vec::new(),
            };

            for contract in &node.contracts {
                let expr = contract.expression.0.clone();
                match contract.kind {
                    ContractKind::Before => record.before.push(expr),
                    ContractKind::After => record.after.push(expr),
                    ContractKind::Always => record.always.push(expr),
                }
            }

            entries.insert(key, record);
        }

        Self { entries }
    }

    /// Detect breaking changes by comparing `self` (the new build) against
    /// `baseline` (the previous verified build).
    ///
    /// A change is breaking if:
    /// - A function present in `baseline` is absent in `self` (removed).
    /// - A contract present in `baseline` is absent in `self` (contract
    ///   removed — pre/post condition relaxed or postcondition dropped).
    /// - A contract absent in `baseline` is present in `self` (contract
    ///   added — precondition tightened or new postcondition imposed).
    ///
    /// # Limitation (v0.1)
    ///
    /// Detection uses raw string equality. Semantically equivalent expressions
    /// that differ in whitespace will be reported as changes. Phase 4 parser
    /// normalization will improve this.
    pub fn breaking_changes(&self, baseline: &ContractSummary) -> Vec<BreakingChange> {
        let mut changes = Vec::new();

        // Functions removed from baseline
        for (name, base_record) in &baseline.entries {
            match self.entries.get(name) {
                None => {
                    changes.push(BreakingChange::FunctionRemoved { name: name.clone() });
                }
                Some(new_record) => {
                    diff_contracts(
                        &mut changes,
                        name,
                        "before",
                        &base_record.before,
                        &new_record.before,
                    );
                    diff_contracts(
                        &mut changes,
                        name,
                        "after",
                        &base_record.after,
                        &new_record.after,
                    );
                    diff_contracts(
                        &mut changes,
                        name,
                        "always",
                        &base_record.always,
                        &new_record.always,
                    );
                }
            }
        }

        // Contracts added in self that were not in baseline
        for (name, new_record) in &self.entries {
            if let Some(base_record) = baseline.entries.get(name) {
                add_contracts(
                    &mut changes,
                    name,
                    "before",
                    &base_record.before,
                    &new_record.before,
                );
                add_contracts(
                    &mut changes,
                    name,
                    "after",
                    &base_record.after,
                    &new_record.after,
                );
                add_contracts(
                    &mut changes,
                    name,
                    "always",
                    &base_record.always,
                    &new_record.always,
                );
            }
            // new functions (not in baseline) are NOT breaking
        }

        changes
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Push `ContractRemoved` entries for expressions in `baseline` missing from `new`.
fn diff_contracts(
    changes: &mut Vec<BreakingChange>,
    function: &str,
    kind: &str,
    baseline: &[String],
    new: &[String],
) {
    for expr in baseline {
        if !new.contains(expr) {
            changes.push(BreakingChange::ContractRemoved {
                function: function.to_string(),
                kind: kind.to_string(),
                expr: expr.clone(),
            });
        }
    }
}

/// Push `ContractAdded` entries for expressions in `new` missing from `baseline`.
fn add_contracts(
    changes: &mut Vec<BreakingChange>,
    function: &str,
    kind: &str,
    baseline: &[String],
    new: &[String],
) {
    for expr in new {
        if !baseline.contains(expr) {
            changes.push(BreakingChange::ContractAdded {
                function: function.to_string(),
                kind: kind.to_string(),
                expr: expr.clone(),
            });
        }
    }
}
