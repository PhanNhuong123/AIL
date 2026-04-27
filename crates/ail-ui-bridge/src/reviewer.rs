//! Phase 16.4 — Reviewer (coverage scoring) scheduling and cancellation.
//!
//! Mirrors the `verifier.rs` / `sheaf.rs` two-phase lock + cancellation pattern.
//! `run_reviewer` runs the 3-stage pipeline (parse → validate → type_check),
//! resolves a node id, and calls `compute_coverage` from `ail-coverage` off-lock.
//! Results are projected into `CoverageCompletePayload` and emitted on the
//! `coverage-complete` event.
//!
//! `load_project` silently aborts any in-flight reviewer run without emitting.
//! The frontend must call `cancelReviewerRun` before `loadProject` if it needs
//! the `coverage-complete{cancelled:true}` signal.
//!
//! # Invariants
//!
//! - **16.4-A (separate fence)**: `reviewer_cancelled` is a distinct
//!   `Arc<AtomicBool>` from the verifier and sheaf fences so all three analyses
//!   may legitimately overlap.
//!
//! - **16.4-B (project-agnostic provider cell)**: `reviewer_provider_cell` is
//!   NOT cleared on `load_project`. The ONNX model load is expensive and
//!   reusable across projects.
//!
//! - **16.4-I (provider off-lock)**: the `Arc<OnceLock<...>>` is cloned under a
//!   brief lock, then `get_or_init` is called OFF-LOCK. The `BridgeState` mutex
//!   is NEVER held across `OnnxEmbeddingProvider::new()` or `compute_coverage`.
//!
//! - **16.4-L (string run_id)**: `run_id` is always a `String` on the wire to
//!   defeat JS `Number` precision loss above 2^53.
//!
//! - **16.4-M (no AIL-Cxxx codes)**: error paths emit `ok: false, status:
//!   "Unavailable"` with `missing_concepts: []`; no internal error codes leak
//!   into the payload.
//!
//! - **16.4-N (cancelled omitted when false)**: `skip_serializing_if = "is_false"`
//!   on `CoverageCompletePayload::cancelled`.
//!
//! - **16.4-R (path-like node_id)**: the emitted `node_id` is PATH-LIKE,
//!   translated via `IdMap::get_path()`. Falls back to UUID string on lookup miss.

#![cfg(all(feature = "tauri-commands", feature = "embeddings"))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Runtime, State};

use ail_coverage::compute_coverage;
use ail_graph::cic::CoverageConfig;
use ail_graph::NodeId;

use crate::commands::BridgeState;
use crate::errors::BridgeError;
use crate::events::COVERAGE_COMPLETE;
use crate::ids::IdMap;
use crate::pipeline::load_typed_from_path;
use crate::types::reviewer_result::{CoverageCompletePayload, ReviewerCancelResult};

// ---------------------------------------------------------------------------
// Pure helpers — public and testable without a Tauri runtime
// ---------------------------------------------------------------------------

/// Seed a 64-bit nonce from `SystemTime::now_ns ^ pid`. Deterministic per
/// process startup, unique enough across processes. Mirrors `seed_verifier_nonce`.
pub fn seed_reviewer_nonce() -> u64 {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    ns ^ (std::process::id() as u64)
}

/// Reserve the next reviewer run id string.
///
/// Uses hex-encoded `"reviewer-{seq}-{nonce}"` format so the wire value is
/// clearly distinct from agent, verifier, and sheaf run ids. The String type
/// defeats JS `Number` precision loss above 2^53. Increments seq via
/// `wrapping_add(1)`.
pub fn next_reviewer_run_id_string(seq: &mut u64, nonce: u64) -> String {
    *seq = seq.wrapping_add(1);
    format!("reviewer-{:x}-{:x}", *seq, nonce)
}

/// Project coverage result fields into a `CoverageCompletePayload`.
///
/// Translates the raw `NodeId` UUID to a PATH-LIKE node id via `IdMap::get_path()`,
/// falling back to `node_id.to_string()` on lookup miss (invariant 16.4-R).
/// Truncates `info.missing_aspects` to the top 3 labels. Maps `info.status.label()`
/// to the status string.
pub(crate) fn project_to_coverage_payload(
    typed: &ail_types::TypedGraph,
    result: ail_coverage::CoverageResult,
    run_id: String,
    node_id: NodeId,
) -> CoverageCompletePayload {
    let cfg = CoverageConfig::default();
    let config_hash = cfg.config_hash();
    let info = result.into_info(&cfg, config_hash);

    let id_map = IdMap::build(typed.graph());
    let path = id_map.get_path(node_id);
    let node_id_str = if path.is_empty() {
        node_id.to_string()
    } else {
        path.to_string()
    };

    let status = info.status.label().to_string();
    let score = info.score.map(|s| s as f64);
    let missing_concepts: Vec<String> = info
        .missing_aspects
        .iter()
        .take(3)
        .map(|m| m.concept.clone())
        .collect();

    CoverageCompletePayload {
        run_id,
        ok: true,
        status,
        score,
        node_id: node_id_str,
        missing_concepts,
        empty_parent: info.empty_parent,
        degenerate_basis_fallback: info.degenerate_basis_fallback,
        cancelled: false,
    }
}

/// Build a `CoverageCompletePayload` for any error / unavailable path.
///
/// Invariant 16.4-M: no AIL-Cxxx internal codes in the payload.
fn unavailable_payload(run_id: String, node_id_str: String) -> CoverageCompletePayload {
    CoverageCompletePayload {
        run_id,
        ok: false,
        status: "Unavailable".to_string(),
        score: None,
        node_id: node_id_str,
        missing_concepts: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
        cancelled: false,
    }
}

/// Resolve a path-like node spec to a `NodeId`.
///
/// Tries `IdMap::get_id` first (path-like lookup), then falls back to UUID parse.
/// Returns `None` when neither lookup succeeds.
fn resolve_node_id(id_map: &IdMap, spec: &str) -> Option<NodeId> {
    // 1. Path-like lookup via reverse map.
    if let Some(id) = id_map.get_id(spec) {
        return Some(id);
    }
    // 2. UUID parse fallback (spec may already be a UUID).
    spec.parse::<NodeId>().ok()
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Tauri command: run a single reviewer (coverage scoring) pass for a node.
///
/// `node_id` must be supplied as a path-like string (e.g.
/// `"wallet_service.src.transfer.s1_validate"`). Returns the stringified
/// `run_id` so the caller can pass it to `cancel_reviewer_run` if needed.
///
/// The command follows a three-phase lock pattern (identical to `run_verifier`):
/// 1. Lock → capture context + reserve run_id + validate node_id → drop lock.
/// 1b. Clone provider cell Arc under brief lock, drop lock, call `get_or_init` off-lock.
/// 2. Spawn blocking task OFF-LOCK (pipeline + coverage are sync/blocking).
/// 3. Lock → store task handle → drop lock.
#[tauri::command]
pub async fn run_reviewer<R: Runtime>(
    node_id: Option<String>,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<String, BridgeError> {
    // Phase 1: lock, capture context, validate, generate run_id, drop lock.
    let project_path;
    let captured_generation;
    let run_id;
    let fence: Arc<AtomicBool>;
    let node_id_spec: String;
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        project_path = inner
            .project_path
            .clone()
            .ok_or_else(|| BridgeError::InvalidInput {
                reason: "no project loaded".to_string(),
            })?;
        // node_id is required for reviewer runs.
        node_id_spec = node_id.ok_or_else(|| BridgeError::InvalidInput {
            reason: "node_id is required for run_reviewer".to_string(),
        })?;
        captured_generation = inner.load_generation;
        let nonce = inner.reviewer_id_nonce;
        run_id = next_reviewer_run_id_string(&mut inner.reviewer_run_seq, nonce);
        fence = inner.reviewer_cancelled.clone();
        // Reset fence for the new run.
        fence.store(false, Ordering::SeqCst);
    }

    // Phase 1b: clone provider cell Arc under brief lock, then get_or_init OFF-LOCK.
    let cell_arc: Arc<
        std::sync::OnceLock<Option<Arc<ail_search::OnnxEmbeddingProvider>>>,
    > = {
        let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        inner.reviewer_provider_cell.clone()
    };

    // get_or_init off-lock: expensive ONNX model load runs at most once per process.
    let provider_opt: Option<Arc<ail_search::OnnxEmbeddingProvider>> = cell_arc
        .get_or_init(|| {
            let model_dir = match ail_search::OnnxEmbeddingProvider::ensure_model() {
                Ok(dir) => dir,
                Err(e) => {
                    log::warn!("[reviewer] model not available: {e}");
                    return None;
                }
            };
            match ail_search::OnnxEmbeddingProvider::new(&model_dir) {
                Ok(p) => Some(Arc::new(p)),
                Err(e) => {
                    log::warn!("[reviewer] failed to load ONNX provider: {e}");
                    None
                }
            }
        })
        .clone();

    let run_id_task = run_id.clone();
    let app_task = app.clone();
    let node_id_task = node_id_spec.clone();

    // Phase 2: spawn blocking task (pipeline + coverage are sync/blocking).
    let task = tokio::task::spawn_blocking(move || {
        // Resolve parse dir the same way load_project does.
        let (_, parse_dir) = crate::commands::resolve_project_layout(&project_path);

        // Run parse → validate → type_check (NO verify).
        let typed = match load_typed_from_path(&parse_dir) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[reviewer] pipeline error: {e}");
                if fence.load(Ordering::SeqCst) {
                    return;
                }
                let payload = unavailable_payload(run_id_task, node_id_task);
                let _ = app_task.emit(COVERAGE_COMPLETE, &payload);
                return;
            }
        };

        // Cancel fence check after pipeline.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        // Resolve path-like node_id to UUID NodeId.
        let id_map = IdMap::build(typed.graph());
        let resolved_id = match resolve_node_id(&id_map, &node_id_task) {
            Some(id) => id,
            None => {
                log::warn!("[reviewer] node not found: '{}'", node_id_task);
                if fence.load(Ordering::SeqCst) {
                    return;
                }
                let payload = unavailable_payload(run_id_task, node_id_task);
                let _ = app_task.emit(COVERAGE_COMPLETE, &payload);
                return;
            }
        };

        // Check provider availability.
        let provider = match &provider_opt {
            Some(p) => p.as_ref(),
            None => {
                log::warn!("[reviewer] ONNX provider not available for run {run_id_task}");
                if fence.load(Ordering::SeqCst) {
                    return;
                }
                let id_path = id_map.get_path(resolved_id);
                let node_id_str = if id_path.is_empty() {
                    resolved_id.to_string()
                } else {
                    id_path.to_string()
                };
                let payload = unavailable_payload(run_id_task, node_id_str);
                let _ = app_task.emit(COVERAGE_COMPLETE, &payload);
                return;
            }
        };

        // Fence check before the potentially long coverage computation.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        // Run coverage computation.
        let result = compute_coverage(typed.graph(), provider, resolved_id, &[]);

        // Final fence check before emit.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        let payload = match result {
            Ok(coverage_result) => {
                project_to_coverage_payload(&typed, coverage_result, run_id_task, resolved_id)
            }
            Err(e) => {
                log::warn!("[reviewer] coverage error: {e}");
                let id_path = id_map.get_path(resolved_id);
                let node_id_str = if id_path.is_empty() {
                    resolved_id.to_string()
                } else {
                    id_path.to_string()
                };
                unavailable_payload(run_id_task, node_id_str)
            }
        };

        if let Err(e) = app_task.emit(COVERAGE_COMPLETE, &payload) {
            log::warn!("[reviewer] emit failed: {e}");
            let _ = e;
        }
    });

    // Phase 3: re-lock, store handle (generation-guarded).
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        if inner.load_generation == captured_generation {
            inner.reviewer_run = Some(task);
        } else {
            // Generation advanced — stale run; abort immediately.
            task.abort();
        }
    }

    Ok(run_id)
}

/// Tauri command: cancel the active reviewer run. Sets the cancelled fence,
/// aborts the task handle, emits `coverage-complete` with `cancelled: true`
/// using the PARAMETER `run_id` (invariant 16.4-B8), then resets the fence.
#[tauri::command]
pub fn cancel_reviewer_run<R: Runtime>(
    run_id: String,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<ReviewerCancelResult, BridgeError> {
    let fence: Arc<AtomicBool>;
    let handle: Option<tokio::task::JoinHandle<()>>;
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        fence = inner.reviewer_cancelled.clone();
        handle = inner.reviewer_run.take();
    }

    // Set fence FIRST so any concurrent task iteration exits before emitting.
    fence.store(true, Ordering::SeqCst);
    if let Some(h) = handle {
        h.abort();
    }

    // Emit cancelled payload using the caller-supplied run_id (invariant B8).
    let payload = CoverageCompletePayload {
        run_id: run_id.clone(),
        ok: false,
        status: "Unavailable".to_string(),
        score: None,
        node_id: String::new(),
        missing_concepts: vec![],
        empty_parent: false,
        degenerate_basis_fallback: false,
        cancelled: true,
    };
    if let Err(e) = app.emit(COVERAGE_COMPLETE, &payload) {
        log::warn!("[reviewer] cancel emit failed: {e}");
        let _ = e;
    }

    // Reset fence so future run_reviewer calls start clean.
    fence.store(false, Ordering::SeqCst);

    Ok(ReviewerCancelResult {
        cancelled: true,
        run_id,
    })
}
