//! Agent subprocess manager — Phase 16 task 16.1.
//!
//! Spawns `python -m ail_agent` with `--json-events` enabled, reads JSON
//! envelopes line-by-line from stdout, and forwards them as Tauri events
//! (`agent-step`, `agent-message`, `agent-complete`) to the right-side
//! ChatPanel.
//!
//! # Invariants
//!
//! - **16.1-A (send-time context)**: the caller constructs
//!   [`AgentRunRequest`] by reading `selection` / `path` / `activeLens` /
//!   `chatMode` at dispatch time. This module does not cache context.
//!
//! - **16.1-B (four-layer cancel guard)**:
//!   1. Reader loop checks `RunHandle.cancelled: AtomicBool` before every emit.
//!      `cancel_agent_run` sets this flag FIRST.
//!   2. Reader also compares `inner.agent_run.run_id == own_run_id` under the
//!      state lock; mismatch ⇒ return without emit (belt-and-suspenders).
//!   3. `cancel_agent_run` calls `start_kill` + aborts the reader task + emits
//!      `agent-complete{status:"cancelled"}` AFTER releasing the lock.
//!   4. Frontend listeners compare `payload.runId === currentRunId` before
//!      mutating chat stores (see `+page.svelte`).
//!
//! - **16.1-C (preview apply pipeline)**: enforced on the frontend. Preview
//!   `GraphPatchJson` payloads carried in [`crate::types::agent::AgentPreview`]
//!   are applied through `applyGraphPatch` only.
//!
//! `run_agent` follows a two-phase lock pattern to avoid holding a
//! `MutexGuard` across an `.await` point: (1) lock → reserve `run_id` → drop
//! lock, (2) spawn child + reader task off-lock (spawn is synchronous),
//! (3) lock → store `RunHandle` → drop lock.

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::commands::{BridgeState, BridgeStateInner};
use crate::errors::BridgeError;
use crate::events::{AGENT_COMPLETE, AGENT_MESSAGE, AGENT_STEP};
use crate::types::agent::{
    AgentCancelResult, AgentCompletePayload, AgentMessagePayload, AgentRunRequest, AgentStepPayload,
};

/// Parsed agent event — one variant per JSON envelope `type`. Short-lived
/// per stdout line; the variant size asymmetry comes from the optional
/// `GraphPatchJson` inside `AgentMessagePayload.preview`, which is the
/// whole point of the preview card (carrying a real patch avoids a
/// round-trip). Boxing would add a heap allocation per line for no
/// structural gain.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AgentEvent {
    Step(AgentStepPayload),
    Message(AgentMessagePayload),
    Complete(AgentCompletePayload),
}

/// Per-run handle stored in [`BridgeStateInner::agent_run`]. Dropping this
/// aborts the reader task and force-kills the child process as a safety net
/// — `cancel_agent_run` does this explicitly but `Drop` handles panics and
/// the "new run supersedes old run" path.
pub struct RunHandle {
    pub run_id: String,
    pub cancelled: Arc<AtomicBool>,
    pub cancel_tx: Option<oneshot::Sender<()>>,
    pub child: Child,
    pub reader_task: JoinHandle<()>,
}

impl Drop for RunHandle {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.child.start_kill();
        self.reader_task.abort();
    }
}

/// Seed the per-process nonce used to obfuscate the monotonic `agent_run_seq`
/// on the wire. Ensures a new process cannot collide run-id values with a
/// surviving subprocess from a prior session (mitigates 3a-1 HIGH #2).
pub fn seed_nonce() -> u64 {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    ns ^ (std::process::id() as u64)
}

/// Reserve the next run id. Serialized as a decimal string on the wire to
/// avoid JS `number` precision loss at 2^53 (mitigates red-team B-1).
pub fn next_run_id_string(inner: &mut BridgeStateInner) -> String {
    inner.agent_run_seq = inner.agent_run_seq.wrapping_add(1);
    (inner.agent_run_seq ^ inner.agent_id_nonce).to_string()
}

/// Pure envelope parser. Returns `None` on non-JSON stdout lines (Python
/// tracebacks, `warnings.warn`, partial bytes that somehow slipped through
/// `BufReader::lines`) so the reader loop can `log::warn!` and continue.
pub fn parse_event_line(line: &str) -> Option<AgentEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str::<AgentEvent>(trimmed).ok()
}

/// Pure env-var builder for the subprocess. Injects the four send-time
/// context fields as `AIL_AGENT_*` vars. Does NOT forward any `*_API_KEY` —
/// the Python side reads provider keys from its own env.
pub fn build_request_env(req: &AgentRunRequest) -> Vec<(String, String)> {
    let lens_str = serde_json::to_string(&req.lens)
        .unwrap_or_else(|_| "\"structure\"".to_string())
        .trim_matches('"')
        .to_string();
    let mode_str = serde_json::to_string(&req.mode)
        .unwrap_or_else(|_| "\"ask\"".to_string())
        .trim_matches('"')
        .to_string();
    vec![
        (
            "AIL_AGENT_SELECTION_KIND".to_string(),
            req.selection_kind.clone(),
        ),
        (
            "AIL_AGENT_SELECTION_ID".to_string(),
            req.selection_id.clone().unwrap_or_default(),
        ),
        ("AIL_AGENT_PATH".to_string(), req.path.join("|")),
        ("AIL_AGENT_LENS".to_string(), lens_str),
        ("AIL_AGENT_MODE".to_string(), mode_str),
    ]
}

/// Spawn the agent subprocess with JSON-events mode.
///
/// Dev mode (`AIL_DEV=1`): invokes `python -m ail_agent` directly.
/// Bundle mode: invokes the wrapper script resolved by
/// [`crate::sidecar::resolve_agent_wrapper_path`] (invariant 16.5-D).
///
/// `spawn()` is synchronous on `tokio::process::Command` — no `.await`.
/// The 16.1-B four-layer cancel guard is unchanged: `RunHandle`, `AtomicBool`
/// fence, oneshot, and reader_loop operate on the returned `Child` regardless
/// of which spawn path was taken.
fn spawn_agent_child<R: Runtime>(
    req: &AgentRunRequest,
    run_id: &str,
    app: &AppHandle<R>,
) -> Result<Child, BridgeError> {
    let mut cmd = if crate::sidecar::parse_ail_dev_mode() {
        let mut c = Command::new("python");
        c.arg("-m").arg("ail_agent");
        c
    } else {
        let wrapper = crate::sidecar::resolve_agent_wrapper_path(app)?;
        Command::new(wrapper)
    };
    cmd.arg(&req.text)
        .arg("--json-events")
        .arg("--run-id")
        .arg(run_id);
    if let Some(model) = req.model.as_deref() {
        cmd.arg("--model").arg(model);
    }
    for (k, v) in build_request_env(req) {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd.spawn().map_err(|e| BridgeError::InvalidInput {
        reason: format!("agent spawn failed: {e}"),
    })
}

/// Return the `run_id` field carried by a parsed envelope, regardless of
/// the variant. Used by the reader to cross-check against the run_id Rust
/// generated (defends against a misbehaving sidecar that echoes a wrong id).
fn event_run_id(event: &AgentEvent) -> &str {
    match event {
        AgentEvent::Step(p) => &p.run_id,
        AgentEvent::Message(p) => &p.run_id,
        AgentEvent::Complete(p) => &p.run_id,
    }
}

/// Synthesize a terminal `agent-complete{status:"error"}` envelope and emit
/// it. Used by the reader loop when stdout reaches EOF without ever seeing
/// a `complete` envelope (e.g. the Python child segfaulted or was killed
/// externally before it could emit). Without this, the frontend
/// `isAgentRunning` / `currentRunId` stores would remain set forever.
async fn emit_synthetic_complete<R: Runtime>(
    app: &AppHandle<R>,
    run_id: &str,
    error: Option<String>,
) {
    let payload = AgentCompletePayload {
        run_id: run_id.to_string(),
        status: "error".to_string(),
        error: Some(error.unwrap_or_else(|| "agent exited without emitting complete".to_string())),
    };
    // Clear the handle under lock so a new run can claim agent_run.
    {
        let state = app.state::<BridgeState>();
        let lock_result = state.lock();
        if let Ok(mut inner) = lock_result {
            if inner
                .agent_run
                .as_ref()
                .map(|h| h.run_id == run_id)
                .unwrap_or(false)
            {
                inner.agent_run.take();
            }
        }
    }
    if let Err(e) = app.emit(AGENT_COMPLETE, &payload) {
        #[cfg(feature = "tauri-commands")]
        log::warn!("[agent] synthetic complete emit failed: {e}");
        let _ = e;
    }
}

/// Reader task: streams JSON lines from the child's stdout, applies the
/// four-step emit guard per line (invariant 16.1-B layers 1-3), and emits
/// Tauri events after releasing the state lock. On EOF / read error
/// without a prior `complete` envelope, synthesizes a terminal
/// `agent-complete{status:"error"}` so the frontend never gets stuck.
async fn reader_loop<R: Runtime>(
    app: AppHandle<R>,
    stdout: tokio::process::ChildStdout,
    mut cancel_rx: oneshot::Receiver<()>,
    cancelled: Arc<AtomicBool>,
    run_id: String,
) {
    let mut lines = BufReader::new(stdout).lines();
    loop {
        // Layer 1 fence: cancel signal breaks out of the next_line() await
        // even when no stdout traffic is flowing. The `biased` makes us poll
        // the cancel branch first on every iteration.
        let next = tokio::select! {
            biased;
            _ = &mut cancel_rx => return,
            line = lines.next_line() => line,
        };
        let line = match next {
            Ok(Some(l)) => l,
            Ok(None) => break, // EOF — fall through to synthetic complete.
            Err(e) => {
                #[cfg(feature = "tauri-commands")]
                log::warn!("[agent] stdout read error: {e}");
                if !cancelled.load(Ordering::SeqCst) {
                    emit_synthetic_complete(&app, &run_id, Some(format!("{e}"))).await;
                }
                return;
            }
        };
        // Layer 2 fence: cancelled after scheduling but before parse.
        if cancelled.load(Ordering::SeqCst) {
            return;
        }
        let Some(event) = parse_event_line(&line) else {
            #[cfg(feature = "tauri-commands")]
            log::warn!("[agent] ignoring non-JSON stdout line");
            continue;
        };
        // Cross-check: the envelope's runId must match the run_id Rust
        // generated. A misbehaving sidecar that echoes a different id would
        // pass the frontend guard if we forwarded blindly.
        if event_run_id(&event) != run_id {
            #[cfg(feature = "tauri-commands")]
            log::warn!(
                "[agent] envelope run_id {:?} does not match expected {:?}; dropping",
                event_run_id(&event),
                run_id,
            );
            continue;
        }
        // Layer 3 fence: compare run_id under state lock, clear handle on
        // Complete so the next run can claim agent_run cleanly.
        let is_complete = matches!(event, AgentEvent::Complete(_));
        {
            let state = app.state::<BridgeState>();
            let mut inner = match state.lock() {
                Ok(g) => g,
                Err(_) => {
                    #[cfg(feature = "tauri-commands")]
                    log::warn!("[agent] state lock poisoned; dropping event");
                    return;
                }
            };
            if cancelled.load(Ordering::SeqCst) {
                return;
            }
            let matches = inner
                .agent_run
                .as_ref()
                .map(|h| h.run_id == run_id)
                .unwrap_or(false);
            if !matches {
                return;
            }
            if is_complete {
                inner.agent_run.take();
            }
            // Lock drops at end of scope.
        }
        // Emit AFTER lock release (mirrors watcher 15.11-A step 6).
        let emit_result = match &event {
            AgentEvent::Step(p) => app.emit(AGENT_STEP, p),
            AgentEvent::Message(p) => app.emit(AGENT_MESSAGE, p),
            AgentEvent::Complete(p) => app.emit(AGENT_COMPLETE, p),
        };
        if let Err(e) = emit_result {
            #[cfg(feature = "tauri-commands")]
            log::warn!("[agent] emit failed: {e}");
            let _ = e;
        }
        if is_complete {
            return;
        }
    }
    // EOF reached without a `complete` envelope. The only way to exit the
    // loop body without returning is `Ok(None)` (clean EOF). Synthesize
    // the terminal event so the frontend can clear isAgentRunning.
    if !cancelled.load(Ordering::SeqCst) {
        emit_synthetic_complete(&app, &run_id, None).await;
    }
}

/// Tauri command: run a single agent turn. Returns the stringified `run_id`
/// so the caller can pass it to `cancel_agent_run` later.
#[tauri::command]
pub async fn run_agent<R: Runtime>(
    req: AgentRunRequest,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<String, BridgeError> {
    // Phase 1: reserve run_id under lock, drop lock before spawn.
    let run_id = {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        // Supersede any previous run explicitly: flag + drop handle.
        if let Some(prev) = inner.agent_run.take() {
            prev.cancelled.store(true, Ordering::SeqCst);
            drop(prev); // Drop impl handles start_kill + abort.
        }
        next_run_id_string(&mut inner)
    };

    // Phase 2: spawn child + reader task OFF-LOCK.
    // On spawn failure, emit a synthetic AGENT_COMPLETE{status:"error"} so the
    // frontend `isAgentRunning` / `currentRunId` stores reset, then return
    // Ok(run_id) — the emitted event is the single signal source (C2 fix).
    // Returning Err here would create two simultaneous signals (rejected promise
    // + event), and if the promise rejection races ahead of the event listener
    // binding, `isAgentRunning` stays true permanently.
    let mut child = match spawn_agent_child(&req, &run_id, &app) {
        Ok(c) => c,
        Err(e) => {
            let payload = AgentCompletePayload {
                run_id: run_id.clone(),
                status: "error".to_string(),
                error: Some(format!("spawn failed: {e}")),
            };
            let _ = app.emit(AGENT_COMPLETE, &payload);
            return Ok(run_id);
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "agent stdout pipe missing".to_string(),
        })?;
    // Drain stderr to the log; we never route it to events.
    if let Some(stderr) = child.stderr.take() {
        let run_id_for_stderr = run_id.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                #[cfg(feature = "tauri-commands")]
                log::warn!("[agent {run_id_for_stderr} stderr] {line}");
                let _ = (&run_id_for_stderr, &line);
            }
        });
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let reader_task = tokio::spawn(reader_loop(
        app.clone(),
        stdout,
        cancel_rx,
        cancelled.clone(),
        run_id.clone(),
    ));

    // Phase 3: re-lock to store handle.
    {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        inner.agent_run = Some(RunHandle {
            run_id: run_id.clone(),
            cancelled,
            cancel_tx: Some(cancel_tx),
            child,
            reader_task,
        });
    }

    Ok(run_id)
}

/// Tauri command: cancel the active run if it matches `run_id`. Sets the
/// cancelled fence FIRST, then tears down the handle and emits a terminal
/// `agent-complete{status:"cancelled"}` after the lock is released.
#[tauri::command]
pub fn cancel_agent_run<R: Runtime>(
    run_id: String,
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<AgentCancelResult, BridgeError> {
    let handle_opt = {
        let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        let matches = inner
            .agent_run
            .as_ref()
            .map(|h| h.run_id == run_id)
            .unwrap_or(false);
        if !matches {
            return Ok(AgentCancelResult { cancelled: false });
        }
        // Layer 1 fence (16.1-B): set cancelled BEFORE releasing the lock so
        // any concurrent reader iteration that re-acquires the lock observes
        // it and exits before emitting.
        if let Some(h) = inner.agent_run.as_ref() {
            h.cancelled.store(true, Ordering::SeqCst);
        }
        inner.agent_run.take()
    };

    if let Some(mut h) = handle_opt {
        if let Some(tx) = h.cancel_tx.take() {
            let _ = tx.send(());
        }
        let _ = h.child.start_kill();
        h.reader_task.abort();
        let payload = AgentCompletePayload {
            run_id: run_id.clone(),
            status: "cancelled".to_string(),
            error: None,
        };
        if let Err(e) = app.emit(AGENT_COMPLETE, &payload) {
            #[cfg(feature = "tauri-commands")]
            log::warn!("[agent] cancel emit failed: {e}");
            let _ = e;
        }
    }

    Ok(AgentCancelResult { cancelled: true })
}
