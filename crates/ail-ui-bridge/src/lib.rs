//! `ail-ui-bridge` — JSON serialization bridge from `VerifiedGraph` to the
//! AIL Tauri IDE.
//!
//! This crate is read-only over `VerifiedGraph`. It serializes the verified
//! graph into stable JSON shapes consumed by the SvelteKit frontend, rolls
//! up node status (worst-child), derives stable path-like node IDs, and
//! emits incremental `GraphPatchJson` diffs.
//!
//! ## Features
//!
//! - `tauri-commands` — enables [`commands`] with Tauri command handlers.
//!   Off by default; `cargo build --workspace` never pulls in Tauri.
//!
//! ## Key entry points
//!
//! - [`pipeline::load_verified_from_path`] — run the 4-stage pipeline.
//! - [`serialize::serialize_graph`] — serialize a `VerifiedGraph` to [`types::GraphJson`].
//! - [`serialize::diff_graph`] — compute an incremental diff between two graphs.
//! - [`lens::compute_lens_metrics`] — compute per-lens metrics for a scope.
//! - [`flowchart::build_flowchart`] — build a `FlowchartJson` for a function.

pub mod errors;
pub mod events;
pub mod flowchart;
pub mod ids;
pub mod lens;
pub mod pipeline;
pub mod rollup;
pub mod serialize;
pub mod types;

#[cfg(feature = "tauri-commands")]
pub mod agent;
#[cfg(feature = "tauri-commands")]
pub mod commands;
#[cfg(feature = "tauri-commands")]
pub mod verifier;
#[cfg(feature = "tauri-commands")]
pub mod watcher;

#[cfg(feature = "tauri-commands")]
pub use commands::{
    compute_lens_metrics as compute_lens_metrics_tauri, get_handler, new_bridge_state, BridgeState,
    BridgeStateInner,
};

pub use errors::BridgeError;
pub use lens::compute_lens_metrics;
pub use pipeline::load_verified_from_path;
pub use serialize::{diff_graph, serialize_graph};
pub use types::{
    agent::{
        AgentCancelResult, AgentCompletePayload, AgentMessagePayload, AgentMode, AgentPreview,
        AgentRunRequest, AgentStepPayload,
    },
    flowchart::{FlowEdgeJson, FlowNodeJson, FlowNodeKind, FlowchartJson},
    graph_json::{
        ClusterJson, ErrorRefJson, ExternalJson, FunctionJson, GraphJson, IssueJson, ModuleJson,
        ProjectJson, RelationJson, StepJson, TypeRefJson,
    },
    lens_stats::{Lens, LensStats},
    node_detail::{
        CodeBlob, CounterexampleDetail, InheritedRule, NodeDetail, ReceivesEntry, ReturnsEntry,
        RuleEntry, RuleSource, VerificationDetail,
    },
    patch::{FunctionPatchEntry, FunctionRemoval, GraphPatchJson, StepPatchEntry, StepRemoval},
    status::Status,
    verify_result::{
        VerifyCancelResult, VerifyCompletePayload, VerifyFailureJson, VerifyResultJson,
    },
};
