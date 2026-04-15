//! Typed input and output structs for all five MCP tools.

use serde::{Deserialize, Serialize};

// ── ail.search ───────────────────────────────────────────────────────────────

/// Input for [`ail.search`](crate::tools::run_search).
#[derive(Debug, Deserialize)]
pub struct SearchInput {
    pub query: String,
    /// Maximum number of results to return. Defaults to 10.
    pub budget: Option<usize>,
}

/// Output from `ail.search`.
#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub results: Vec<SearchItem>,
    pub total: usize,
}

/// A single ranked search result.
#[derive(Debug, Serialize)]
pub struct SearchItem {
    pub node_id: String,
    pub score: f32,
    pub intent: String,
    pub pattern: String,
    pub path: Vec<String>,
}

// ── ail.context ──────────────────────────────────────────────────────────────

/// Input for [`ail.context`](crate::tools::run_context).
#[derive(Debug, Deserialize)]
pub struct ContextInput {
    pub task: String,
    /// Hard cap on output size in approximate tokens (words × 1.3). Defaults to 4096.
    pub budget_tokens: Option<usize>,
}

/// Output from `ail.context`.
#[derive(Debug, Serialize)]
pub struct ContextOutput {
    /// Top 70% of ranked nodes — full CIC context packet.
    pub primary: Vec<ContextNode>,
    /// Remaining 30% — intent summary only.
    pub secondary: Vec<ContextSummary>,
    /// Contract strings from primary packets (inherited + call contracts).
    pub contracts: Vec<String>,
}

/// A fully expanded context node (primary result).
#[derive(Debug, Serialize)]
pub struct ContextNode {
    pub node_id: String,
    pub intent: String,
    pub intent_chain: Vec<String>,
    pub scope: Vec<ScopeEntry>,
    pub constraints: Vec<String>,
}

/// A scope variable entry within a context node.
#[derive(Debug, Serialize)]
pub struct ScopeEntry {
    pub name: String,
    pub constraint: String,
}

/// A lightweight secondary result (intent only).
#[derive(Debug, Serialize)]
pub struct ContextSummary {
    pub node_id: String,
    pub intent: String,
}

// ── ail.verify ───────────────────────────────────────────────────────────────

/// Input for [`ail.verify`](crate::tools::run_verify).
#[derive(Debug, Deserialize)]
pub struct VerifyInput {
    /// Path hint — v0.1 always verifies the whole project regardless.
    pub file: Option<String>,
}

/// Output from `ail.verify`.
#[derive(Debug, Serialize)]
pub struct VerifyOutput {
    pub ok: bool,
    pub errors: Vec<String>,
    /// `true` when only static contract checks were run (Z3 feature not enabled).
    pub static_checks_only: bool,
}

// ── ail.build ────────────────────────────────────────────────────────────────

/// Input for [`ail.build`](crate::tools::run_build).
#[derive(Debug, Deserialize)]
pub struct BuildInput {
    /// Emission target (v0.1: always "python").
    pub target: Option<String>,
    /// Inject contract checks into emitted code. Defaults to `true`.
    pub contracts: Option<bool>,
    /// Emit `async` function signatures. Defaults to `false`.
    pub async_mode: Option<bool>,
}

/// Output from `ail.build`.
#[derive(Debug, Serialize)]
pub struct BuildOutput {
    pub ok: bool,
    pub files: Vec<BuildFile>,
    pub errors: Vec<String>,
}

/// A single emitted file.
#[derive(Debug, Serialize)]
pub struct BuildFile {
    pub path: String,
    /// `"Generated"` (always overwrite) or `"Scaffolded"` (create once).
    pub ownership: String,
    pub size_bytes: usize,
}

// ── ail.status ───────────────────────────────────────────────────────────────

/// Output from `ail.status`.
#[derive(Debug, Serialize)]
pub struct StatusOutput {
    /// The highest pipeline stage reached: "raw", "validated", "typed", or "verified".
    pub pipeline_stage: String,
    pub node_count: usize,
    pub edge_count: usize,
    /// Number of `Do` pattern nodes (each must have contracts).
    pub do_node_count: usize,
}
