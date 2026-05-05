//! `ail-ui-bridge` — JSON serialization bridge from `VerifiedGraph` (or
//! `TypedGraph`) to the AIL Tauri IDE.
//!
//! This crate is read-only over graph types. It serializes graphs into stable
//! JSON shapes consumed by the SvelteKit frontend, rolls up node status
//! (worst-child), derives stable path-like node IDs, and emits incremental
//! `GraphPatchJson` diffs.
//!
//! ## Features
//!
//! - `tauri-commands` — enables [`commands`] with Tauri command handlers.
//!   Off by default; `cargo build --workspace` never pulls in Tauri.
//!
//! ## Key entry points
//!
//! - [`pipeline::load_typed_from_path`] — run the 3-stage pipeline (parse +
//!   validate + type_check); used by `load_project` and watcher cycles.
//! - [`pipeline::load_verified_from_path`] — run the full 4-stage pipeline
//!   (adds contract verification); used by `verify_project`.
//! - [`serialize::serialize_typed_graph`] — serialize a `TypedGraph` to [`types::GraphJson`].
//! - [`serialize::serialize_graph`] — serialize a `VerifiedGraph` to [`types::GraphJson`].
//! - [`serialize::diff_graph`] — compute an incremental diff between two graphs.
//! - [`lens::compute_lens_metrics`] — compute per-lens metrics for a scope.
//! - [`flowchart::build_flowchart`] — build a `FlowchartJson` for a function.

pub mod errors;
pub mod events;
pub mod flowchart;
pub mod ids;
pub mod layout;
pub mod lens;
pub mod pipeline;
pub mod rollup;
pub mod save_registry;
pub mod serialize;
pub mod types;

#[cfg(feature = "tauri-commands")]
pub mod agent;
#[cfg(feature = "tauri-commands")]
pub mod commands;
#[cfg(all(feature = "tauri-commands", feature = "embeddings"))]
pub mod reviewer;
#[cfg(feature = "tauri-commands")]
pub mod scaffold;
#[cfg(feature = "tauri-commands")]
pub mod sheaf;
#[cfg(feature = "tauri-commands")]
pub mod sidecar;
#[cfg(feature = "tauri-commands")]
pub mod tutorial;
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
pub use layout::{load_layout, merge_and_save, save_layout, LayoutError, LAYOUT_FILE, SIDECAR_DIR};
pub use lens::compute_lens_metrics;
pub use save_registry::{
    save_registry, SaveContext, SaveRegistry, SaveSource, SessionId, ECHO_WINDOW, MAX_AGE,
};
pub use pipeline::load_typed_from_path;
pub use pipeline::load_verified_from_path;
pub use serialize::{diff_graph, serialize_graph, serialize_typed_graph};
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
    reviewer_result::{CoverageCompletePayload, ReviewerCancelResult},
    scaffold::{ProjectScaffoldRequest, ProjectScaffoldResult},
    sheaf::{SheafCancelResult, SheafCompletePayload, SheafConflictEntry},
    sidecar_result::{HealthCheckPayload, SidecarMode},
    status::Status,
    verify_result::{
        VerifyCancelResult, VerifyCompletePayload, VerifyFailureJson, VerifyResultJson,
    },
};
