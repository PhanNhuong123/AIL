//! Tauri command handlers for the AIL IDE.
//!
//! All items in this module are gated behind the `tauri-commands` feature.
//! The module is not compiled in default builds, keeping `cargo build --workspace`
//! Tauri-free.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::State;

use crate::errors::BridgeError;
use crate::flowchart::build_flowchart;
use crate::ids::IdMap;
use crate::pipeline::{load_verified_from_path, read_project_name};
use crate::serialize::serialize_graph;
use crate::types::flowchart::FlowchartJson;
use crate::types::graph_json::GraphJson;
use crate::types::node_detail::NodeDetail;
use crate::types::verify_result::VerifyResultJson;

/// Shared Tauri application state for the bridge.
pub struct BridgeStateInner {
    pub project_path: Option<PathBuf>,
    pub graph_json: Option<GraphJson>,
}

/// Mutex-wrapped bridge state managed by Tauri.
pub type BridgeState = Mutex<BridgeStateInner>;

/// Create a new, empty `BridgeState`.
pub fn new_bridge_state() -> BridgeState {
    Mutex::new(BridgeStateInner {
        project_path: None,
        graph_json: None,
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
fn resolve_project_layout(supplied: &Path) -> (PathBuf, PathBuf) {
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
    inner.project_path = Some(root);
    inner.graph_json = Some(graph_json.clone());

    Ok(graph_json)
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
        .ok_or_else(|| BridgeError::NodeNotFound { id: node_id })
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

/// Validate that `function_id` resolves to a function node. Persistence is
/// deferred to a later task (16.9).
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

    // Persistence is deferred to task 16.9.
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
    ]
}
