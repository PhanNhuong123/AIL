//! Phase 17.4 — Sheaf analysis scheduling and cancellation.
//!
//! Mirrors the `verifier.rs` two-phase lock + cancellation pattern. `run_sheaf_analysis`
//! runs the 3-stage pipeline (parse → validate → type_check, NOT verify) and then,
//! when `z3-verify` is enabled, calls `analyze_sheaf_obstructions` to detect H1
//! obstructions in the Čech nerve. Results are projected into `SheafConflictEntry`
//! values and emitted on the `sheaf-complete` event.
//!
//! `load_project` does NOT emit `sheaf-complete` when it cancels an in-flight sheaf
//! run. The frontend is responsible for calling `cancelSheafAnalysis` before
//! triggering `loadProject` if it needs the `sheaf-complete{cancelled:true}` signal.
//! Within `load_project`, the sheaf run is aborted and the fence is reset silently.
//!
//! # Invariants
//!
//! - **17.4-A (two-phase lock)**: `run_sheaf_analysis` follows the same three-phase
//!   pattern as `run_verifier`: (1) lock → capture context + generate run_id → drop
//!   lock, (2) spawn blocking task OFF-LOCK, (3) lock → store handle → drop lock.
//!   The pipeline is sync/blocking and MUST NOT be called while holding the mutex.
//!
//! - **17.4-B (fence-guarded emit)**: every emit inside the spawned task is preceded
//!   by a fence check (`sheaf_cancelled.load(SeqCst)`). The task exits silently when
//!   the fence is set.
//!
//! - **17.4-C (load_generation guard)**: if the project was reloaded between the
//!   Phase 1 lock drop and the Phase 3 store, the task handle is aborted immediately.
//!
//! - **17.4-D (default-features path)**: when the `z3-verify` feature is absent,
//!   `run_sheaf_analysis` emits `SheafCompletePayload { ok: true, z3_available: false,
//!   conflicts: vec![], cancelled: false, error: None }` so the frontend listener
//!   fires reliably.
//!
//! - **17.4-E (scope filtering)**: when `node_id: Some(id)` is provided, the nerve
//!   is filtered to the BFS subtree rooted at that node before obstruction detection.
//!   Only obstructions where both endpoints are in the filtered nerve are reported.
//!
//! - **17.4-F (cancel emit)**: `cancel_sheaf_analysis` MUST emit
//!   `sheaf-complete{cancelled: true}` (invariant 17.4-F). `load_project` abort is
//!   silent — the frontend calls `cancelSheafAnalysis` first.

#![cfg(feature = "tauri-commands")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Runtime, State};

use crate::commands::BridgeState;
use crate::errors::BridgeError;
use crate::events::SHEAF_COMPLETE;
use crate::pipeline::load_typed_from_path;
use crate::types::sheaf::{SheafCancelResult, SheafCompletePayload};

#[cfg(feature = "z3-verify")]
use crate::types::sheaf::SheafConflictEntry;

// ---------------------------------------------------------------------------
// Pure helpers — public and testable without a Tauri runtime
// ---------------------------------------------------------------------------

/// Seed a 64-bit nonce from `SystemTime::now_ns ^ pid`. Deterministic per
/// process startup, unique enough across processes. Mirrors `seed_verifier_nonce`.
pub fn seed_sheaf_nonce() -> u64 {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    ns ^ (std::process::id() as u64)
}

/// Reserve the next sheaf run id string.
///
/// Uses hex-encoded `"sheaf-{seq}-{nonce}"` format so the wire value is
/// clearly distinct from verifier and agent run ids. The String type defeats
/// JS `Number` precision loss above 2^53.
pub fn next_sheaf_run_id_string(seq: &mut u64, nonce: u64) -> String {
    *seq = seq.wrapping_add(1);
    format!("sheaf-{:x}-{:x}", *seq, nonce)
}

// ---------------------------------------------------------------------------
// Z3-gated helper: project ObstructionResult → SheafConflictEntry
// ---------------------------------------------------------------------------

/// Project a slice of `ObstructionResult` into `SheafConflictEntry` values.
///
/// Only `Contradictory` entries are projected; `Consistent` and `Unknown` are
/// silently skipped. Constraint expressions are stringified via their `Display`
/// impl.
///
/// `id_map` is used to translate raw `NodeId` UUIDs into path-like step IDs
/// (e.g. `"wallet_service.src.transfer.s1_validate"`) so that the frontend can
/// match conflict entries directly against `StepJson.id` values in `GraphJson`.
/// Entries where either side has no path-like ID (should not occur for nodes in
/// a valid typed graph, but defended against) are skipped.
///
/// Gated behind `z3-verify` because `ObstructionResult` / `ObstructionStatus`
/// only exist under that feature.
#[cfg(feature = "z3-verify")]
pub fn project_to_sheaf_conflicts(
    obstructions: &[ail_contract::ObstructionResult],
    id_map: &crate::ids::IdMap,
) -> Vec<SheafConflictEntry> {
    use ail_contract::ObstructionStatus;

    obstructions
        .iter()
        .filter_map(|r| {
            if let ObstructionStatus::Contradictory {
                conflicting_a,
                conflicting_b,
            } = &r.status
            {
                let a_path = id_map.get_path(r.node_a);
                let b_path = id_map.get_path(r.node_b);
                if a_path.is_empty() || b_path.is_empty() {
                    // Defensive: skip entries with no path mapping.
                    return None;
                }
                Some(SheafConflictEntry {
                    overlap_index: r.overlap_index,
                    node_a: a_path.to_string(),
                    node_b: b_path.to_string(),
                    conflicting_a: conflicting_a.iter().map(|c| format!("{c}")).collect(),
                    conflicting_b: conflicting_b.iter().map(|c| format!("{c}")).collect(),
                })
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Tauri command: run a sheaf analysis pass over the loaded project.
///
/// When `node_id` is `Some`, the Čech nerve is filtered to the BFS subtree
/// rooted at that node before obstruction detection. When the `z3-verify`
/// feature is absent, the command emits a successful payload with
/// `z3_available: false` and an empty conflict list.
///
/// Returns the stringified `run_id` so the caller can pass it to
/// `cancel_sheaf_analysis` if needed.
///
/// The command follows a two-phase lock pattern (identical to `run_verifier`):
/// 1. Lock → capture context + reserve run_id → drop lock.
/// 2. Spawn blocking pipeline task OFF-LOCK.
/// 3. Lock → store task handle → drop lock.
#[tauri::command]
pub async fn run_sheaf_analysis<R: Runtime>(
    node_id: Option<String>,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<String, BridgeError> {
    // Phase 1: lock, capture context, generate run_id, drop lock.
    let project_path;
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
        captured_generation = inner.load_generation;
        let nonce = inner.sheaf_id_nonce;
        run_id = next_sheaf_run_id_string(&mut inner.sheaf_run_seq, nonce);
        fence = inner.sheaf_cancelled.clone();
        // Reset fence for the new run.
        fence.store(false, Ordering::SeqCst);
    }

    let run_id_task = run_id.clone();
    let app_task = app.clone();

    // Phase 2: spawn blocking task (pipeline is sync/blocking).
    let task = tokio::task::spawn_blocking(move || {
        // Resolve parse dir the same way load_project does.
        let (_, parse_dir) = crate::commands::resolve_project_layout(&project_path);

        // Run parse → validate → type_check (NO verify).
        let typed_result = load_typed_from_path(&parse_dir);

        // Cancel fence check before any analysis or emit.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        let (ok, z3_available, conflicts, error) = match typed_result {
            Err(e) => {
                log::warn!("[sheaf] pipeline error: {e}");
                (
                    false,
                    cfg!(feature = "z3-verify"),
                    vec![],
                    Some(e.to_string()),
                )
            }
            Ok(typed) => {
                // Run obstruction detection under z3-verify; emit empty payload otherwise.
                #[cfg(feature = "z3-verify")]
                {
                    use ail_contract::analyze_sheaf_obstructions;
                    use ail_contract::filter_to_subtree;
                    use std::str::FromStr;

                    let (nerve, obstructions) = analyze_sheaf_obstructions(&typed);

                    // Cancel fence check after potentially long Z3 pass.
                    if fence.load(Ordering::SeqCst) {
                        return;
                    }

                    // Apply subtree scope filter if node_id was supplied.
                    let filtered_obstructions = if let Some(ref id_str) = node_id {
                        match ail_graph::NodeId::from_str(id_str) {
                            Ok(root_id) => {
                                let filtered_nerve =
                                    filter_to_subtree(&nerve, root_id, typed.graph());
                                // Keep only obstructions where both endpoints are in the filtered nerve.
                                let in_scope: std::collections::HashSet<String> = filtered_nerve
                                    .sections
                                    .iter()
                                    .map(|s| s.node_id.to_string())
                                    .collect();
                                obstructions
                                    .into_iter()
                                    .filter(|r| {
                                        in_scope.contains(&r.node_a.to_string())
                                            && in_scope.contains(&r.node_b.to_string())
                                    })
                                    .collect::<Vec<_>>()
                            }
                            Err(e) => {
                                log::warn!(
                                    "[sheaf] invalid node_id '{id_str}', ignoring scope filter: {e}"
                                );
                                obstructions
                            }
                        }
                    } else {
                        obstructions
                    };

                    let id_map = crate::ids::IdMap::build(typed.graph());
                    let conflicts = project_to_sheaf_conflicts(&filtered_obstructions, &id_map);
                    (true, true, conflicts, None)
                }

                #[cfg(not(feature = "z3-verify"))]
                {
                    // Z3 not compiled in; emit success with empty conflicts and z3_available=false.
                    let _ = typed;
                    let _ = node_id;
                    (true, false, vec![], None)
                }
            }
        };

        // Final fence check before emit.
        if fence.load(Ordering::SeqCst) {
            return;
        }

        let payload = SheafCompletePayload {
            run_id: run_id_task.clone(),
            ok,
            z3_available,
            conflicts,
            cancelled: false,
            error,
        };
        if let Err(e) = app_task.emit(SHEAF_COMPLETE, &payload) {
            log::warn!("[sheaf] emit failed: {e}");
            let _ = e;
        }
    });

    // Phase 3: re-lock, store handle (generation-guarded).
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        if inner.load_generation == captured_generation {
            inner.sheaf_run = Some(task);
        } else {
            // Generation advanced — stale run; abort immediately.
            task.abort();
        }
    }

    Ok(run_id)
}

/// Tauri command: cancel the active sheaf analysis run. Sets the cancelled fence,
/// aborts the task handle, emits `sheaf-complete` with `cancelled: true`, then
/// resets the fence (invariant 17.4-F).
#[tauri::command]
pub fn cancel_sheaf_analysis<R: Runtime>(
    run_id: String,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<SheafCancelResult, BridgeError> {
    let fence: Arc<AtomicBool>;
    let handle: Option<tokio::task::JoinHandle<()>>;
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        fence = inner.sheaf_cancelled.clone();
        handle = inner.sheaf_run.take();
    }

    // Set fence FIRST so any concurrent task iteration exits before emitting.
    fence.store(true, Ordering::SeqCst);
    if let Some(h) = handle {
        h.abort();
    }

    let payload = SheafCompletePayload {
        run_id: run_id.clone(),
        ok: false,
        z3_available: cfg!(feature = "z3-verify"),
        conflicts: vec![],
        cancelled: true,
        error: None,
    };
    if let Err(e) = app.emit(SHEAF_COMPLETE, &payload) {
        log::warn!("[sheaf] cancel emit failed: {e}");
        let _ = e;
    }

    // Reset fence so future run_sheaf_analysis calls start clean.
    fence.store(false, Ordering::SeqCst);

    Ok(SheafCancelResult { cancelled: true })
}
