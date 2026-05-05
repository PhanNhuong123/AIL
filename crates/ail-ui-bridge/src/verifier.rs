//! Phase 16.3 — Verifier scheduling and cancellation.
//!
//! Mirrors the `agent.rs` two-phase lock + cancellation pattern. `run_verifier`
//! re-runs the AIL pipeline on demand (via `load_verified_from_path`),
//! diff-emits `graph-updated` so the existing patch pipeline updates the
//! frontend's graph, then emits a `verify-complete` event with run/scope
//! metadata. `load_project` actively cancels any in-flight run via a
//! dedicated `Arc<AtomicBool>` fence + `task.abort()`.
//!
//! # Invariants
//!
//! - **16.3-A (cancel-on-reload)**: `load_project` sets the verifier fence,
//!   aborts the task handle, then resets the fence — exactly mirroring the
//!   watcher teardown pattern.
//!
//! - **16.3-B (two-phase lock)**: `run_verifier` follows the same three-phase
//!   pattern as `run_agent`: (1) lock → reserve run_id → drop lock,
//!   (2) spawn blocking task OFF-LOCK, (3) lock → store handle → drop lock.
//!   The pipeline (`load_verified_from_path`) is sync/blocking and MUST NOT
//!   be called while holding the `BridgeState` mutex.
//!
//! - **16.3-C (outcome classification, v4.0)**: `VerifyFailureJson.outcome` and
//!   `VerificationDetail.outcome` carry per-node Z3 verdicts. When the verified
//!   pipeline fails, the verifier re-runs the typed pipeline and
//!   `verify_contracts` directly, classifies each `VerifyError` into a
//!   [`VerifyOutcome`], and overlays the verdicts onto the typed-graph JSON via
//!   [`apply_verify_outcomes`]. Requires the `z3-verify` feature; without it
//!   the failure path keeps the legacy "no detail" behaviour.

#![cfg(feature = "tauri-commands")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Runtime, State};

use crate::commands::BridgeState;
use crate::errors::BridgeError;
use crate::events::{GRAPH_UPDATED, VERIFY_COMPLETE};
use crate::pipeline::load_verified_from_path;
use crate::serialize::{diff_graph, serialize_graph};
use crate::types::graph_json::GraphJson;
use crate::types::verify_result::{VerifyCancelResult, VerifyCompletePayload, VerifyFailureJson};

#[cfg(feature = "z3-verify")]
use std::collections::HashMap;
#[cfg(feature = "z3-verify")]
use ail_contract::{verify_contracts, VerifyError};
#[cfg(feature = "z3-verify")]
use crate::pipeline::load_typed_from_path;
#[cfg(feature = "z3-verify")]
use crate::serialize::{apply_verify_outcomes, serialize_typed_graph, NodeVerdict};
#[cfg(feature = "z3-verify")]
use crate::types::node_detail::{CounterexampleDetail, VerifyOutcome};

// ---------------------------------------------------------------------------
// Pure helpers — public and testable without a Tauri runtime
// ---------------------------------------------------------------------------

/// Seed a 64-bit nonce from `SystemTime::now_ns ^ pid`. Deterministic per
/// process startup, unique enough across processes.
pub fn seed_verifier_nonce() -> u64 {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    ns ^ (std::process::id() as u64)
}

/// Reserve the next verifier run id string.
///
/// Uses a hex-encoded `"verify-{seq}-{nonce}"` format so the wire value is
/// clearly distinct from agent run ids. The String type defeats JS `Number`
/// precision loss above 2^53.
pub fn next_verifier_run_id_string(seq: &mut u64, nonce: u64) -> String {
    *seq = seq.wrapping_add(1);
    format!("verify-{:x}-{:x}", *seq, nonce)
}

/// Collect the set of node ids that fall within a verification scope.
///
/// Scope semantics:
/// - `"project"` → all module, function, and step ids.
/// - `"module"` + `scope_id` → the module itself, its functions, and their
///   steps. Returns empty vec if `scope_id` is `None` or the module is not
///   found (mirrors the TypeScript `collect-scope-ids.ts` contract:
///   "Unknown scopeId → empty array (defensive)").
/// - `"function"` + `scope_id` → the function, its parent module, and the
///   function's steps. Returns empty vec if `scope_id` is `None` or not found.
/// - `"step"` + `scope_id` → the step, its parent function, and the
///   grandparent module. Returns empty vec if `scope_id` is `None` or not
///   found.
/// - Any other scope value → empty vec (defensive, matches TS `default: []`).
pub fn collect_scope_ids(graph: &GraphJson, scope: &str, scope_id: Option<&str>) -> Vec<String> {
    match scope {
        "project" => collect_all_ids(graph),
        "module" => {
            let sid = match scope_id {
                Some(s) => s,
                None => return vec![],
            };
            for module in &graph.modules {
                if module.id == sid {
                    let mut ids = vec![module.id.clone()];
                    for func in &module.functions {
                        ids.push(func.id.clone());
                        if let Some(steps) = &func.steps {
                            for step in steps {
                                ids.push(step.id.clone());
                            }
                        }
                    }
                    return ids;
                }
            }
            // scope_id provided but not found in graph → empty (defensive)
            vec![]
        }
        "function" => {
            let sid = match scope_id {
                Some(s) => s,
                None => return vec![],
            };
            for module in &graph.modules {
                for func in &module.functions {
                    if func.id == sid {
                        let mut ids = vec![module.id.clone(), func.id.clone()];
                        if let Some(steps) = &func.steps {
                            for step in steps {
                                ids.push(step.id.clone());
                            }
                        }
                        return ids;
                    }
                }
            }
            // scope_id provided but not found in graph → empty (defensive)
            vec![]
        }
        "step" => {
            let sid = match scope_id {
                Some(s) => s,
                None => return vec![],
            };
            for module in &graph.modules {
                for func in &module.functions {
                    if let Some(steps) = &func.steps {
                        for step in steps {
                            if step.id == sid {
                                return vec![module.id.clone(), func.id.clone(), step.id.clone()];
                            }
                        }
                    }
                }
            }
            // scope_id provided but not found in graph → empty (defensive)
            vec![]
        }
        // Unknown scope value → empty (matches TS `default: []`)
        _ => vec![],
    }
}

/// Collect every module, function, and step id in the graph.
fn collect_all_ids(graph: &GraphJson) -> Vec<String> {
    let mut ids = Vec::new();
    for module in &graph.modules {
        ids.push(module.id.clone());
        for func in &module.functions {
            ids.push(func.id.clone());
            if let Some(steps) = &func.steps {
                for step in steps {
                    ids.push(step.id.clone());
                }
            }
        }
    }
    ids
}

/// Return true if all nine patch arrays are empty (no-op diff).
fn patch_is_empty(patch: &crate::types::patch::GraphPatchJson) -> bool {
    patch.modules_added.is_empty()
        && patch.modules_modified.is_empty()
        && patch.modules_removed.is_empty()
        && patch.functions_added.is_empty()
        && patch.functions_modified.is_empty()
        && patch.functions_removed.is_empty()
        && patch.steps_added.is_empty()
        && patch.steps_modified.is_empty()
        && patch.steps_removed.is_empty()
}

/// Build `VerifyFailureJson` items from `GraphJson.issues` where severity is
/// `"fail"`. `outcome` is forwarded from the issue (populated by
/// [`apply_verify_outcomes`] when a Z3 verdict is available; `None` otherwise).
fn failures_from_graph(graph: &GraphJson) -> Vec<VerifyFailureJson> {
    graph
        .issues
        .iter()
        .filter(|issue| issue.severity.as_deref() == Some("fail"))
        .map(|issue| VerifyFailureJson {
            node_id: issue.node_id.clone(),
            message: issue.message.clone(),
            stage: issue.stage.clone(),
            severity: issue.severity.clone(),
            source: issue.source.clone(),
            outcome: issue.outcome.clone(),
        })
        .collect()
}

/// Classify a single [`VerifyError`] into a [`NodeVerdict`].
///
/// Mapping (per `crates/ail-contract/src/errors/verify_error.rs`):
/// - `AIL-C013 SolverTimeout` → `Timeout`.
/// - `AIL-C014 EncodingFailed`, `AIL-C010 UnsatTypeConstraints` → `Unknown`
///   (no usable counterexample to surface).
/// - `AIL-C011 ContradictoryPreconditions`, `AIL-C012 PostconditionNotEntailed`,
///   `AIL-C015 PromotedFactContradiction` → `Unsat` with the Z3 model captured
///   in `CounterexampleDetail`.
#[cfg(feature = "z3-verify")]
fn classify_verify_error(err: &VerifyError) -> NodeVerdict {
    match err {
        VerifyError::SolverTimeout { contract_expr, .. } => NodeVerdict {
            outcome: VerifyOutcome::Timeout,
            counterexample: Some(CounterexampleDetail {
                scenario: String::new(),
                effect: format!("solver timed out checking `{contract_expr}`"),
                violates: contract_expr.clone(),
            }),
        },
        VerifyError::EncodingFailed { inner, .. } => NodeVerdict {
            outcome: VerifyOutcome::Unknown,
            counterexample: Some(CounterexampleDetail {
                scenario: String::new(),
                effect: format!("contract encoding failed: {inner}"),
                violates: String::new(),
            }),
        },
        VerifyError::UnsatTypeConstraints { .. } => NodeVerdict {
            outcome: VerifyOutcome::Unknown,
            counterexample: Some(CounterexampleDetail {
                scenario: String::new(),
                effect: "parameter types are mutually contradictory".to_string(),
                violates: String::new(),
            }),
        },
        VerifyError::ContradictoryPreconditions {
            counterexample, ..
        } => NodeVerdict {
            outcome: VerifyOutcome::Unsat,
            counterexample: Some(CounterexampleDetail {
                scenario: counterexample.clone(),
                effect: "preconditions are jointly contradictory".to_string(),
                violates: String::new(),
            }),
        },
        VerifyError::PostconditionNotEntailed {
            contract_expr,
            counterexample,
            ..
        } => NodeVerdict {
            outcome: VerifyOutcome::Unsat,
            counterexample: Some(CounterexampleDetail {
                scenario: counterexample.clone(),
                effect: format!("postcondition `{contract_expr}` is not entailed"),
                violates: contract_expr.clone(),
            }),
        },
        VerifyError::PromotedFactContradiction {
            counterexample, ..
        } => NodeVerdict {
            outcome: VerifyOutcome::Unsat,
            counterexample: Some(CounterexampleDetail {
                scenario: counterexample.clone(),
                effect: "promoted fact from check node contradicts preconditions".to_string(),
                violates: String::new(),
            }),
        },
    }
}

/// Severity ordering used to merge multiple errors that affect the same node.
/// Higher = retained when collapsing. Timeout wins over counterexamples wins
/// over Unknown so the surfaced verdict reflects the most actionable issue.
#[cfg(feature = "z3-verify")]
fn outcome_priority(outcome: VerifyOutcome) -> u8 {
    match outcome {
        VerifyOutcome::Timeout => 3,
        VerifyOutcome::Unsat => 2,
        VerifyOutcome::Unknown => 1,
        VerifyOutcome::Sat => 0,
    }
}

/// Build a path-keyed `NodeVerdict` map from a list of `VerifyError`s and an
/// `IdMap` covering the typed graph. Multiple errors on the same node are
/// collapsed via [`outcome_priority`].
#[cfg(feature = "z3-verify")]
fn build_verdict_map(
    errors: &[VerifyError],
    id_map: &crate::ids::IdMap,
) -> HashMap<String, NodeVerdict> {
    let mut out: HashMap<String, NodeVerdict> = HashMap::new();
    for err in errors {
        let node_id = match err {
            VerifyError::UnsatTypeConstraints { node_id }
            | VerifyError::ContradictoryPreconditions { node_id, .. }
            | VerifyError::PostconditionNotEntailed { node_id, .. }
            | VerifyError::SolverTimeout { node_id, .. }
            | VerifyError::EncodingFailed { node_id, .. }
            | VerifyError::PromotedFactContradiction { node_id, .. } => *node_id,
        };
        let path = id_map.get_path(node_id).to_string();
        if path.is_empty() {
            continue;
        }
        let candidate = classify_verify_error(err);
        match out.get(&path) {
            Some(existing)
                if outcome_priority(existing.outcome) >= outcome_priority(candidate.outcome) =>
            {
                // Keep the higher-priority verdict already recorded.
            }
            _ => {
                out.insert(path, candidate);
            }
        }
    }
    out
}

/// Re-run the typed pipeline + `verify_contracts` and produce a
/// `(GraphJson, failures)` pair with per-node Z3 verdicts overlaid.
///
/// Used as the failure-path fallback for `run_verifier` so the IDE can render
/// the specific reason a node failed instead of a generic "verification
/// failed". Returns `None` if the typed pipeline itself fails (parse/validate/
/// type-check error) — that case stays in the legacy "no fresh graph" branch.
#[cfg(feature = "z3-verify")]
fn build_failed_graph_with_outcomes(
    project_path: &std::path::Path,
    parse_dir: &std::path::Path,
) -> Option<GraphJson> {
    let typed = match load_typed_from_path(parse_dir) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("[verifier] typed pipeline failed during outcome classification: {e}");
            return None;
        }
    };

    let errors = verify_contracts(&typed);
    let id_map = crate::ids::IdMap::build(typed.graph());
    let verdicts = build_verdict_map(&errors, &id_map);

    let name = crate::pipeline::read_project_name(project_path);
    let mut graph = serialize_typed_graph(&typed, &name);
    apply_verify_outcomes(&mut graph, &verdicts);
    Some(graph)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Tauri command: run a single verifier pass over the loaded project.
///
/// Returns the stringified `run_id` so the caller can pass it to
/// `cancel_verifier_run` if needed.
///
/// The command follows a two-phase lock pattern (identical to `run_agent`):
/// 1. Lock → capture context + reserve run_id → drop lock.
/// 2. Spawn blocking pipeline task OFF-LOCK.
/// 3. Lock → store task handle → drop lock.
#[tauri::command]
pub async fn run_verifier<R: Runtime>(
    scope: String,
    scope_id: Option<String>,
    node_ids: Vec<String>,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<String, BridgeError> {
    // Phase 1: lock, capture context, generate run_id, drop lock.
    let project_path;
    let prev_graph;
    let captured_generation;
    let run_id;
    let fence: Arc<AtomicBool>;
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
        prev_graph = inner
            .graph_json
            .clone()
            .ok_or_else(|| BridgeError::InvalidInput {
                reason: "no project loaded".to_string(),
            })?;
        captured_generation = inner.load_generation;
        let nonce = inner.verifier_id_nonce;
        run_id = next_verifier_run_id_string(&mut inner.verifier_run_seq, nonce);
        fence = inner.verifier_cancelled.clone();
        // Reset fence for the new run.
        fence.store(false, Ordering::SeqCst);
    }

    let run_id_task = run_id.clone();
    let app_task = app.clone();
    let scope_task = scope.clone();
    let scope_id_task = scope_id.clone();
    let node_ids_task = node_ids.clone();

    // Phase 2: spawn blocking task (pipeline is sync).
    let task = tokio::task::spawn_blocking(move || {
        // Resolve parse dir the same way load_project does.
        let (_, parse_dir) = crate::commands::resolve_project_layout(&project_path);

        let result = load_verified_from_path(&parse_dir);

        let (pipeline_ok, fresh_graph_opt) = match result {
            Ok(verified) => {
                let name = crate::pipeline::read_project_name(&project_path);
                let g = serialize_graph(&verified, &name);
                (true, Some(g))
            }
            Err(_e) => {
                log::warn!("[verifier] pipeline error: {_e}");
                // v4.0: when verify fails, re-run the typed pipeline + Z3
                // classifier so the per-node `VerificationDetail.outcome`
                // reaches the IDE. Falls back to no fresh graph if the typed
                // pipeline itself fails (parse/validate/type-check error).
                #[cfg(feature = "z3-verify")]
                let fallback = build_failed_graph_with_outcomes(&project_path, &parse_dir);
                #[cfg(not(feature = "z3-verify"))]
                let fallback = None;
                (false, fallback)
            }
        };

        // Cancel fence check before any emit.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        let failures;
        let ok;

        match fresh_graph_opt {
            Some(fresh_graph) => {
                // Emit graph-updated with the diff so the patch pipeline
                // updates the frontend's in-memory graph.
                let patch = diff_graph(&prev_graph, &fresh_graph);
                if !patch_is_empty(&patch) {
                    let _ = app_task.emit(GRAPH_UPDATED, &patch);
                }
                // Build failures from the fresh graph's issue list.
                failures = failures_from_graph(&fresh_graph);
                ok = pipeline_ok && failures.is_empty();
            }
            None => {
                failures = vec![];
                ok = false;
            }
        }

        // Final fence check before emit.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        let payload = VerifyCompletePayload {
            ok,
            failures,
            run_id: run_id_task.clone(),
            scope: scope_task,
            scope_id: scope_id_task,
            node_ids: node_ids_task,
            cancelled: false,
        };
        let _ = app_task.emit(VERIFY_COMPLETE, &payload);
    });

    // Phase 3: re-lock, store handle (generation-guarded).
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        if inner.load_generation == captured_generation {
            inner.verifier_run = Some(task);
        } else {
            // Generation advanced — stale run; abort immediately.
            task.abort();
        }
    }

    Ok(run_id)
}

/// Tauri command: cancel the active verifier run. Sets the cancelled fence,
/// aborts the task handle, and emits `verify-complete` with `cancelled: true`.
#[tauri::command]
pub fn cancel_verifier_run<R: Runtime>(
    run_id: String,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<VerifyCancelResult, BridgeError> {
    let fence: Arc<AtomicBool>;
    let handle: Option<tokio::task::JoinHandle<()>>;
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        fence = inner.verifier_cancelled.clone();
        handle = inner.verifier_run.take();
    }

    // Set fence FIRST so any concurrent task iteration exits before emitting.
    fence.store(true, Ordering::SeqCst);
    if let Some(h) = handle {
        h.abort();
    }

    let payload = VerifyCompletePayload {
        ok: false, // M1: cancelled run did not pass verification
        failures: vec![],
        run_id: run_id.clone(),
        scope: "cancelled".to_string(),
        scope_id: None,
        node_ids: vec![],
        cancelled: true,
    };
    if let Err(e) = app.emit(VERIFY_COMPLETE, &payload) {
        #[cfg(feature = "tauri-commands")]
        log::warn!("[verifier] cancel emit failed: {e}");
        let _ = e;
    }

    // I1: reset fence so future run_verifier calls start clean.
    fence.store(false, Ordering::SeqCst);

    Ok(VerifyCancelResult { cancelled: true })
}
