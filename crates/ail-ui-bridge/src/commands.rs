//! Tauri command handlers for the AIL IDE.
//!
//! All items in this module are gated behind the `tauri-commands` feature.
//! The module is not compiled in default builds, keeping `cargo build --workspace`
//! Tauri-free.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, FileIdMap};
use tauri::{AppHandle, Runtime, State};

use crate::errors::BridgeError;
use crate::flowchart::build_flowchart;
use crate::ids::IdMap;
use crate::pipeline::{load_verified_from_path, read_project_name};
use crate::serialize::serialize_graph;
use crate::types::flowchart::FlowchartJson;
use crate::types::graph_json::GraphJson;
use crate::types::lens_stats::{Lens, LensStats};
use crate::types::node_detail::NodeDetail;
use crate::types::verify_result::VerifyResultJson;
use crate::watcher;

/// Shared Tauri application state for the bridge.
///
/// `watcher` holds the active `.ail` file-system watcher (one per loaded
/// project). Replacing it via `.take()` on `load_project` is the sole
/// teardown mechanism — `Drop` stops the underlying notify thread.
///
/// `load_generation` is a monotonic token bumped on every `load_project`.
/// Watcher callbacks capture the generation at dispatch and no-op when the
/// generation has advanced, protecting against races where a pending
/// debounced event from the previous project fires after re-load.
pub struct BridgeStateInner {
    pub project_path: Option<PathBuf>,
    pub graph_json: Option<GraphJson>,
    pub watcher: Option<Debouncer<RecommendedWatcher, FileIdMap>>,
    pub load_generation: u64,
}

/// Mutex-wrapped bridge state managed by Tauri.
pub type BridgeState = Mutex<BridgeStateInner>;

/// Create a new, empty `BridgeState`.
pub fn new_bridge_state() -> BridgeState {
    Mutex::new(BridgeStateInner {
        project_path: None,
        graph_json: None,
        watcher: None,
        load_generation: 0,
    })
}

/// Resolve the project root and the parse directory from a caller-supplied path.
///
/// The caller may supply either the project root (where `ail.config.toml` lives)
/// or the `src/` subdirectory inside it. Both conventions are handled:
///
/// 1. `supplied` has `ail.config.toml` **and** a `src/` subdir → root = supplied, parse_dir = src/
/// 2. `supplied` has `ail.config.toml` but no `src/` subdir     → root = supplied, parse_dir = supplied
/// 3. `supplied`'s parent has `ail.config.toml` (legacy src/ call)→ root = parent,  parse_dir = supplied
/// 4. All else → root = supplied, parse_dir = supplied
pub(crate) fn resolve_project_layout(supplied: &Path) -> (PathBuf, PathBuf) {
    if supplied.join("ail.config.toml").is_file() && supplied.join("src").is_dir() {
        (supplied.to_path_buf(), supplied.join("src"))
    } else if supplied.join("ail.config.toml").is_file() {
        (supplied.to_path_buf(), supplied.to_path_buf())
    } else if let Some(parent) = supplied.parent() {
        if parent.join("ail.config.toml").is_file() {
            (parent.to_path_buf(), supplied.to_path_buf())
        } else {
            (supplied.to_path_buf(), supplied.to_path_buf())
        }
    } else {
        (supplied.to_path_buf(), supplied.to_path_buf())
    }
}

/// Load a project from `path`, run the full pipeline, serialize it, and store
/// the result in state. Returns the serialized `GraphJson`.
///
/// `path` should be the **project root** (the directory containing
/// `ail.config.toml`). Passing a `src/` subdirectory is also tolerated for
/// backward compatibility — `resolve_project_layout` detects the convention
/// and derives the correct root and parse directory automatically.
#[tauri::command]
pub fn load_project(path: String, state: State<'_, BridgeState>) -> Result<GraphJson, BridgeError> {
    let (root, parse_dir) = resolve_project_layout(&PathBuf::from(&path));
    let verified = load_verified_from_path(&parse_dir)?;
    let name = read_project_name(&root);
    let graph_json = serialize_graph(&verified, &name);

    let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    // Drop any prior watcher BEFORE mutating path: its callback may still fire
    // otherwise and emit a patch against a stale `graph_json`.
    if let Some(old) = inner.watcher.take() {
        drop(old);
    }
    inner.load_generation = inner.load_generation.wrapping_add(1);
    inner.project_path = Some(root);
    inner.graph_json = Some(graph_json.clone());

    Ok(graph_json)
}

/// Start a `.ail` file-system watcher scoped to the currently loaded project's
/// parse directory. Debounced events fire a full re-pipeline + diff + emit of
/// [`crate::events::GRAPH_UPDATED`] with a [`crate::types::patch::GraphPatchJson`].
///
/// Requires a prior successful `load_project`. A re-invocation replaces the
/// previous watcher — but normally callers only invoke this once per project
/// load; `load_project` tears down the prior watcher by itself.
#[tauri::command]
pub fn start_watch_project<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, BridgeState>,
) -> Result<(), BridgeError> {
    let (parse_dir, generation) = {
        let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
            reason: "state lock poisoned".to_string(),
        })?;
        let root = inner
            .project_path
            .as_ref()
            .ok_or_else(|| BridgeError::InvalidInput {
                reason: "no project loaded".to_string(),
            })?;
        let (_, parse_dir) = resolve_project_layout(root);
        (parse_dir, inner.load_generation)
    };

    // Build the debouncer OUTSIDE the state lock. notify spawns a worker
    // thread during construction; holding the mutex across that would
    // serialize unrelated commands.
    let debouncer = watcher::start_watcher(app, parse_dir, generation)?;

    let mut inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    // If a concurrent `load_project` raced and already replaced the watcher,
    // drop the one we just built rather than clobber a newer entry.
    if inner.load_generation != generation {
        drop(debouncer);
        return Ok(());
    }
    if let Some(old) = inner.watcher.take() {
        drop(old);
    }
    inner.watcher = Some(debouncer);
    Ok(())
}

/// Return the `NodeDetail` for a node identified by `node_id` (path string).
#[tauri::command]
pub fn get_node_detail(
    node_id: String,
    state: State<'_, BridgeState>,
) -> Result<NodeDetail, BridgeError> {
    let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    let graph = inner
        .graph_json
        .as_ref()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "no project loaded".to_string(),
        })?;
    graph
        .detail
        .get(&node_id)
        .cloned()
        .ok_or(BridgeError::NodeNotFound { id: node_id })
}

/// Build and return the flowchart for a function identified by `function_id`.
#[tauri::command]
pub fn get_flowchart(
    function_id: String,
    state: State<'_, BridgeState>,
) -> Result<FlowchartJson, BridgeError> {
    let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    let root = inner
        .project_path
        .as_ref()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "no project loaded".to_string(),
        })?;

    let (_, parse_dir) = resolve_project_layout(root);
    let verified = load_verified_from_path(&parse_dir)?;
    let id_map = IdMap::build(verified.graph());
    build_flowchart(verified.graph(), &id_map, &function_id)
}

/// Re-run the full pipeline and return a verification result.
///
/// MVP: returns `ok=true` with empty failures after a successful pipeline run.
#[tauri::command]
pub fn verify_project(state: State<'_, BridgeState>) -> Result<VerifyResultJson, BridgeError> {
    let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    let root = inner
        .project_path
        .as_ref()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "no project loaded".to_string(),
        })?;

    // Re-run pipeline to confirm still valid.
    let (_, parse_dir) = resolve_project_layout(root);
    let _verified = load_verified_from_path(&parse_dir)?;

    Ok(VerifyResultJson {
        ok: true,
        failures: Vec::new(),
    })
}

/// Compute per-lens metrics for an optional scope within the loaded project.
///
/// `lens` selects the metric projection. `scope_id` restricts computation to a
/// single module, function, or step node (by path id). Pass `None` for
/// project-wide metrics.
#[tauri::command]
pub fn compute_lens_metrics(
    lens: Lens,
    scope_id: Option<String>,
    state: State<'_, BridgeState>,
) -> Result<LensStats, BridgeError> {
    let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    let graph = inner
        .graph_json
        .as_ref()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "no project loaded".to_string(),
        })?;
    Ok(crate::lens::compute_lens_metrics(
        graph,
        lens,
        scope_id.as_deref(),
    ))
}

/// Validate that `function_id` resolves to a function node. Persistence is
/// deferred to a later phase (see canonical v4 roadmap).
#[tauri::command]
pub fn save_flowchart(
    function_id: String,
    _chart: FlowchartJson,
    state: State<'_, BridgeState>,
) -> Result<(), BridgeError> {
    let inner = state.lock().map_err(|_| BridgeError::InvalidInput {
        reason: "state lock poisoned".to_string(),
    })?;
    let root = inner
        .project_path
        .as_ref()
        .ok_or_else(|| BridgeError::InvalidInput {
            reason: "no project loaded".to_string(),
        })?;

    let (_, parse_dir) = resolve_project_layout(root);
    let verified = load_verified_from_path(&parse_dir)?;
    let id_map = IdMap::build(verified.graph());
    if id_map.get_id(&function_id).is_none() {
        return Err(BridgeError::NodeNotFound { id: function_id });
    }

    // Persistence is deferred to a later phase (see canonical v4 roadmap).
    Ok(())
}

/// Return the Tauri command handler list for registration in `src-tauri/main.rs`.
///
/// In Tauri 2 the handler type is generic over the runtime `R`. Register it
/// with `tauri::Builder::default().invoke_handler(get_handler())`.
///
/// Full compilation is verified in task 16.2 when the Tauri shell crate wires
/// this up with a concrete runtime. The signature follows the Tauri 2 public
/// `InvokeHandler<R>` alias:
/// `pub type InvokeHandler<R> = dyn Fn(Invoke<R>) -> bool + Send + Sync + 'static`
pub fn get_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        load_project,
        get_node_detail,
        get_flowchart,
        verify_project,
        save_flowchart,
        compute_lens_metrics,
        start_watch_project,
    ]
}
