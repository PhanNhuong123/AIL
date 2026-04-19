//! Typed input and output structs for all seven MCP tools.

use serde::{Deserialize, Serialize};

// ── ail.search ───────────────────────────────────────────────────────────────

/// Input for the `ail.search` MCP tool.
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
    /// BM25 or RRF score as f32 — kept for backward compatibility.
    pub score: f32,
    pub intent: String,
    pub pattern: String,
    pub path: Vec<String>,
    // Ranking provenance — populated by hybrid_search (Phase 10 gap closure).
    /// Ranking source: `"bm25_only"`, `"semantic_only"`, or `"both"`.
    pub source: String,
    /// Combined RRF score (1/60+rank+1 across sources). Equals `score as f64`.
    pub rrf_score: f64,
    /// 0-based rank in the BM25 result list, or `None` if not a BM25 hit.
    pub bm25_rank: Option<usize>,
    /// 0-based rank in the semantic result list, or `None` if not a semantic hit.
    pub semantic_rank: Option<usize>,
}

// ── ail.context ──────────────────────────────────────────────────────────────

/// Input for the `ail.context` MCP tool.
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
    /// Facts proved by preceding `check X otherwise raise E` nodes (Phase 8).
    /// Empty when no check nodes precede this node in its execution path.
    pub promoted_facts: Vec<PromotedFactEntry>,
}

/// A scope variable entry within a context node.
#[derive(Debug, Serialize)]
pub struct ScopeEntry {
    pub name: String,
    pub constraint: String,
}

/// A single promoted fact in a context node response.
///
/// Represents a condition that a preceding `check` node proved true and that
/// is therefore available as a verified assumption for this node.
#[derive(Debug, Serialize)]
pub struct PromotedFactEntry {
    /// Raw condition expression proved by the check
    /// (e.g. `"sender.balance >= amount"`).
    pub condition: String,
    /// [`NodeId`](ail_graph::NodeId) of the `check` node that established this fact.
    pub source_node_id: String,
    /// Human-readable intent of the check node, when available.
    /// Allows MCP consumers to display `(from check at <intent>)` without an
    /// extra lookup round-trip.
    ///
    /// `None` when the check node is no longer present in the graph at the
    /// time the context packet is rendered (e.g. a stale packet was read from
    /// cache after the check was removed and before invalidation ran). The
    /// `condition` and `source_node_id` fields are still valid in that case.
    pub source_node_intent: Option<String>,
}

/// A lightweight secondary result (intent only).
#[derive(Debug, Serialize)]
pub struct ContextSummary {
    pub node_id: String,
    pub intent: String,
}

// ── ail.verify ───────────────────────────────────────────────────────────────

/// Input for the `ail.verify` MCP tool.
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

/// Input for the `ail.build` MCP tool.
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

// ── ail.write ───────────────────────────────────────────────────────────────

/// Input for the `ail.write` MCP tool — create a new node.
#[derive(Debug, Deserialize)]
pub struct WriteInput {
    /// Node ID of the parent to insert under.
    pub parent_id: String,
    /// AIL pattern: "do", "define", "describe", "check", "let", etc.
    pub pattern: String,
    /// Human-readable intent for the node.
    pub intent: String,
    /// Raw expression text (optional — leaf nodes only).
    pub expression: Option<String>,
    /// 0-based position among siblings. Defaults to appending as last child.
    pub position: Option<usize>,
    /// Contracts to attach to the new node.
    pub contracts: Option<Vec<ContractInput>>,
    /// Optional partial `NodeMetadata` — shallow-merged into the default; enables
    /// creating named nodes (`Define`/`Describe`/`Error`/`Do`), params,
    /// `return_type`, and `following_template_name` in a single write.
    pub metadata: Option<serde_json::Value>,
}

/// A contract supplied through the MCP write/patch interface.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractInput {
    /// Contract kind: "before", "after", or "always".
    pub kind: String,
    /// Raw contract expression text.
    pub expression: String,
}

/// Output from `ail.write`.
#[derive(Debug, Serialize)]
pub struct WriteOutput {
    pub status: String,
    pub node_id: String,
    pub depth: usize,
    pub path: Vec<String>,
    pub auto_edges: Vec<AutoEdgeOutput>,
    pub cic_invalidated: usize,
    pub warnings: Vec<String>,
}

/// An auto-detected Ed edge created after a write or patch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AutoEdgeOutput {
    /// Always `"ed"` for diagonal cross-references.
    pub kind: String,
    /// Node ID of the referenced target.
    pub target: String,
    /// Relationship label: `"uses_type"`, `"raises"`, `"calls"`, or
    /// `"follows_template"`.
    pub label: String,
}

// ── ail.patch ───────────────────────────────────────────────────────────────

/// Input for the `ail.patch` MCP tool — update existing node fields.
#[derive(Debug, Deserialize)]
pub struct PatchInput {
    /// Node ID to patch.
    pub node_id: String,
    /// Fields to update. Only provided fields are changed.
    pub fields: PatchFields,
}

/// Patchable fields for `ail.patch`. All optional — only provided fields are
/// applied. Use `ail.move` for parent/position changes.
#[derive(Debug, Deserialize)]
pub struct PatchFields {
    pub intent: Option<String>,
    pub expression: Option<String>,
    pub pattern: Option<String>,
    /// When provided, replaces all existing contracts on the node.
    pub contracts: Option<Vec<ContractInput>>,
    /// When provided, merged into existing metadata (shallow merge).
    pub metadata: Option<serde_json::Value>,
}

/// Output from `ail.patch`.
#[derive(Debug, Serialize)]
pub struct PatchOutput {
    pub status: String,
    pub node_id: String,
    pub changed_fields: Vec<String>,
    pub auto_edges_added: Vec<AutoEdgeOutput>,
    pub auto_edges_removed: Vec<AutoEdgeOutput>,
    pub cic_invalidated: usize,
    pub warnings: Vec<String>,
}

// ── ail.move ────────────────────────────────────────────────────────────────

/// Input for the `ail.move` MCP tool — restructure a node under a new parent.
#[derive(Debug, Deserialize)]
pub struct MoveInput {
    /// Node ID to move.
    pub node_id: String,
    /// New parent's node ID.
    pub new_parent_id: String,
    /// 0-based position among new siblings. Defaults to appending as last child.
    pub position: Option<usize>,
}

/// Output from `ail.move`.
#[derive(Debug, Serialize)]
pub struct MoveOutput {
    pub status: String,
    pub node_id: String,
    /// `None` when moving a node that previously had no parent (a root).
    pub old_parent_id: Option<String>,
    pub new_parent_id: String,
    pub old_depth: usize,
    pub new_depth: usize,
    /// Number of descendants that moved with the node.
    pub descendants_moved: usize,
    pub cic_invalidated: usize,
    pub warnings: Vec<String>,
}

// ── ail.delete ──────────────────────────────────────────────────────────────

/// Input for the `ail.delete` MCP tool.
#[derive(Debug, Deserialize)]
pub struct DeleteInput {
    /// Node ID to delete.
    pub node_id: String,
    /// Strategy: `"cascade"` (default), `"orphan"`, or `"dry_run"`.
    pub strategy: Option<String>,
}

/// Output from `ail.delete`. Field population depends on the chosen strategy.
#[derive(Debug, Serialize)]
pub struct DeleteOutput {
    /// `"deleted"`, `"orphaned"`, or `"dry_run"`.
    pub status: String,
    /// Number of nodes actually removed (0 for `dry_run`).
    pub deleted_nodes: usize,
    /// IDs of nodes actually removed (empty for `dry_run`).
    pub deleted_node_ids: Vec<String>,
    /// For `dry_run` only — how many nodes a cascade would remove.
    pub would_delete: usize,
    /// For `dry_run` only — IDs a cascade would remove.
    pub would_delete_ids: Vec<String>,
    /// For `orphan` only — how many direct children were lifted to the parent.
    pub reparented_children: usize,
    /// Count of incident Ed edges removed (or that would be, for `dry_run`).
    pub affected_ed_edges: usize,
    pub cic_invalidated: usize,
    pub warnings: Vec<String>,
}

// ── ail.batch ───────────────────────────────────────────────────────────────

/// Input for the `ail.batch` MCP tool — execute a sequence of ordered graph
/// mutations atomically.
///
/// The entire batch runs inside a single in-memory transaction: if any
/// operation fails, the graph is restored to its pre-batch state and the
/// failure is reported. After all operations succeed, auto-edge detection is
/// re-run across every affected node so references added or removed by earlier
/// operations are reflected in the final graph state.
#[derive(Debug, Deserialize)]
pub struct BatchInput {
    /// Operations to apply in order.
    pub operations: Vec<BatchOperation>,
}

/// A single entry in a batch. The `op` discriminator selects which tool the
/// body is deserialized as.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BatchOperation {
    Write(WriteInput),
    Patch(PatchInput),
    Move(MoveInput),
    Delete(DeleteInput),
}

/// Output from `ail.batch`.
#[derive(Debug, Serialize)]
pub struct BatchOutput {
    /// `"completed"` when every operation succeeded, `"rolled_back"` when the
    /// batch failed and the graph was restored.
    pub status: String,
    /// Per-operation results in the same order as the input. Each entry is a
    /// JSON value matching the corresponding tool's output shape on success,
    /// or an error description for operations that ran before rollback.
    pub results: Vec<BatchOperationResult>,
    /// Total number of Ed (diagonal) edges created or removed by the
    /// post-batch auto-edge refresh pass.
    pub auto_edges_refreshed: usize,
    /// Aggregate CIC cache invalidation count. Always 0 for the in-memory
    /// `AilGraph` backend; reserved for SQLite-backed callers.
    pub total_cic_invalidated: usize,
    /// `None` when the batch completed; contains the failing operation index
    /// and message when the batch was rolled back.
    pub error: Option<String>,
}

/// A single per-operation result inside a [`BatchOutput::results`] list.
#[derive(Debug, Serialize)]
pub struct BatchOperationResult {
    /// The operation discriminator: `"write"`, `"patch"`, `"move"`, or `"delete"`.
    pub op: String,
    /// `"ok"` when the operation succeeded, `"error"` otherwise.
    pub status: String,
    /// Full tool response as JSON when `status == "ok"`; `None` on error.
    pub output: Option<serde_json::Value>,
    /// Error message when `status == "error"`; `None` otherwise.
    pub error: Option<String>,
}

// ── ail.review ───────────────────────────────────────────────────────────────

/// Input for the `ail.review` MCP tool.
#[derive(Debug, Deserialize)]
pub struct ReviewInput {
    /// Node ID (UUID string) or node name to review.
    pub node: String,
}

/// Output from `ail.review`.
#[derive(Debug, Serialize)]
pub struct ReviewOutput {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    /// Coverage score in `[0.0, 1.0]`, or `null` for leaf nodes.
    pub coverage: Option<f32>,
    /// Qualitative status: `"Full"`, `"Partial"`, `"Weak"`, `"N/A"`, or
    /// `"Unavailable"`.
    pub status: String,
    pub children_coverage: Vec<ChildCoverageItem>,
    pub missing: Vec<MissingItem>,
    /// Human-readable action suggestion derived from status and missing aspects.
    pub suggestion: String,
    /// Surfaced only when `true` — parent embedding was near-zero (Guard A).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_parent: Option<bool>,
    /// Surfaced only when `true` — Gram-Schmidt basis degenerated (Guard C).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degenerate_basis_fallback: Option<bool>,
}

/// Per-child contribution entry in a [`ReviewOutput`].
#[derive(Debug, Serialize)]
pub struct ChildCoverageItem {
    pub node_id: String,
    pub intent_preview: String,
    pub contribution: f32,
}

/// A missing semantic aspect in a [`ReviewOutput`].
#[derive(Debug, Serialize)]
pub struct MissingItem {
    pub concept: String,
    pub similarity: f32,
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
    /// UUID string of the graph's root node, or absent when the graph has no
    /// root set (e.g. a freshly-initialised empty project).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_id: Option<String>,
}
