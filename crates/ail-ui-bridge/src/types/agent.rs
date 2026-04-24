//! Agent request / event DTOs — Phase 16 contract.
//!
//! Mirror in `ide/src/lib/types.ts`. All struct fields serialize as camelCase
//! so the TypeScript side receives `runId`, `messageId`, `selectionKind`, etc.
//!
//! `runId` is a **string** on the wire. Internally the Rust side uses a
//! `u64` monotonic counter XOR'd with a session nonce, but every payload
//! serializes the value as `String` so JavaScript's `number` type (IEEE-754
//! double, exact only up to 2^53 − 1) cannot silently collide high-bit ids.
//! The round trip is: Rust `u64 → String → JSON → TS string`.

use serde::{Deserialize, Serialize};

use super::lens_stats::Lens;
use super::patch::GraphPatchJson;

/// Chat mode selected by the user at send time. Serialized in lowercase so
/// the frontend's `'edit' | 'ask' | 'test'` union round-trips as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Edit,
    Ask,
    Test,
}

/// Canonical Send-time context injected into every run (invariant 16.1-A).
///
/// `selection_kind` is a free-form string (`"project" | "module" | "function"
/// | "step" | "type" | "error" | "none"`) to avoid coupling the bridge DTO
/// surface to the frontend's `Selection` discriminated-union wire shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    pub text: String,
    pub selection_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_id: Option<String>,
    #[serde(default)]
    pub path: Vec<String>,
    pub lens: Lens,
    pub mode: AgentMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A planner / coder / verifier step event, emitted many times per run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStepPayload {
    pub run_id: String,
    pub index: u32,
    pub phase: String,
    pub text: String,
}

/// Optional preview card attached to an agent message. The `patch`, when
/// present, is applied through the canonical `applyGraphPatch` pipeline on
/// `Apply` (invariant 16.1-C).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPreview {
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<GraphPatchJson>,
}

/// An assistant message event. `message_id` is a stable identifier the
/// frontend can key on (and the preview card id mirrors it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessagePayload {
    pub run_id: String,
    pub message_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<AgentPreview>,
}

/// Terminal run event. `status` is one of `"done" | "error" | "cancelled"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompletePayload {
    pub run_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response shape for `cancel_agent_run`. `cancelled` is `false` when the
/// caller's `run_id` does not match the currently-tracked run (already
/// completed, superseded, or never existed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCancelResult {
    pub cancelled: bool,
}
