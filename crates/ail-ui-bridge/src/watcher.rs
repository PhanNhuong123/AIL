//! `.ail` file-system watcher — re-runs the pipeline, diffs, and emits a
//! fine-grained [`GraphPatchJson`] via the [`crate::events::GRAPH_UPDATED`]
//! Tauri event.
//!
//! # Invariant 15.11-A
//!
//! - Debounce window ≥ 250 ms (`DEBOUNCE_MS`).
//! - Filename filter: extension `.ail` AND not a known editor temp pattern
//!   (`*.tmp`, `*~`, `.#*`, `*___jb_tmp___`, `*___jb_old___`, `.*.sw?`).
//! - Lock-then-emit protocol:
//!     1. Pipeline runs off-lock (blocking I/O must not hold the mutex).
//!     2. Lock; re-check `load_generation` matches; clone prev; compute patch.
//!     3. Store next graph; drop lock; emit event.
//!
//!   The emit happens after the lock is dropped so concurrent commands that
//!   need the mutex aren't held behind the IPC boundary.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, Debouncer, FileIdMap};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::BridgeState;
use crate::errors::BridgeError;
use crate::events::GRAPH_UPDATED;
use crate::pipeline::{load_verified_from_path, read_project_name};
use crate::serialize::{diff_graph_at, serialize_graph};
use crate::types::graph_json::GraphJson;
use crate::types::patch::GraphPatchJson;

/// Debounce window. Collapses editor-save bursts (tmp+rename+modify on
/// Windows/VSCode) into a single dispatch.
const DEBOUNCE_MS: u64 = 250;

/// Start watching `parse_dir` for `.ail` changes.
///
/// `generation` is the `load_generation` value at the moment the watcher was
/// requested. Each callback dispatch re-reads the state's current generation;
/// if it has advanced, the callback is discarded (a newer project has been
/// loaded).
pub(crate) fn start_watcher<R: Runtime>(
    app: AppHandle<R>,
    parse_dir: PathBuf,
    generation: u64,
) -> Result<Debouncer<RecommendedWatcher, FileIdMap>, BridgeError> {
    let app_cb = app.clone();
    let parse_dir_cb = parse_dir.clone();

    let mut debouncer = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        None,
        move |result: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
            let events = match result {
                Ok(events) => events,
                Err(errs) => {
                    for err in errs {
                        log::warn!("[watcher] notify error: {err}");
                    }
                    return;
                }
            };
            if !events.iter().any(is_relevant_ail_change) {
                return;
            }
            dispatch_cycle(&app_cb, &parse_dir_cb, generation);
        },
    )
    .map_err(|e| BridgeError::InvalidInput {
        reason: format!("watcher init failed: {e}"),
    })?;

    debouncer
        .watcher()
        .watch(&parse_dir, RecursiveMode::Recursive)
        .map_err(|e| BridgeError::InvalidInput {
            reason: format!("watch failed: {e}"),
        })?;

    Ok(debouncer)
}

/// Event filter: accept create / modify / remove on an `.ail` file that is
/// not an editor temp artefact.
fn is_relevant_ail_change(ev: &DebouncedEvent) -> bool {
    matches!(
        ev.event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && ev.event.paths.iter().any(|p| path_is_ail_source(p))
}

pub fn path_is_ail_source(path: &Path) -> bool {
    let is_ail_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ail"))
        .unwrap_or(false);
    if !is_ail_ext {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    !is_editor_temp(name)
}

fn is_editor_temp(name: &str) -> bool {
    // Common editor temp / backup patterns — safer to over-filter than to
    // fire a pipeline run on a half-written file.
    name.ends_with(".tmp")
        || name.ends_with('~')
        || name.starts_with(".#")
        || name.contains("___jb_tmp___")
        || name.contains("___jb_old___")
        || (name.starts_with('.') && (name.ends_with(".swp") || name.ends_with(".swo")))
}

/// Full watcher callback cycle.
///
/// See the module-level invariant 15.11-A for the six-step lock-then-emit
/// protocol. Errors are logged and swallowed — the user is mid-edit and
/// transient parse failures should not surface as bridge errors.
fn dispatch_cycle<R: Runtime>(app: &AppHandle<R>, parse_dir: &Path, generation: u64) {
    // 1. Pipeline off-lock.
    let verified = match load_verified_from_path(parse_dir) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[watcher] pipeline failed: {e}");
            return;
        }
    };

    // Read project name before taking the mutex; read_project_name is pure
    // file I/O and does not need state access.
    let root = parse_dir
        .parent()
        .filter(|p| p.join("ail.config.toml").is_file())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| parse_dir.to_path_buf());
    let name = read_project_name(&root);
    let next_graph = serialize_graph(&verified, &name);

    let patch: GraphPatchJson = {
        let state = app.state::<BridgeState>();
        let mut inner = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                log::warn!("[watcher] state lock poisoned; skipping emit");
                return;
            }
        };
        // 2. Race guard: a newer project may have been loaded during the
        //    off-lock pipeline run. Discard our result in that case.
        if inner.load_generation != generation {
            return;
        }
        // Prev: current graph in state (the one we are diffing against).
        // Must be Some — `start_watch_project` requires prior `load_project`.
        let prev = inner
            .graph_json
            .clone()
            .unwrap_or_else(empty_graph_placeholder);
        // 3. Diff under lock — prev and next are both stable here.
        let patch = diff_graph_at(&prev, &next_graph, current_timestamp_ms());
        // 4. Store the new graph so subsequent commands see it.
        inner.graph_json = Some(next_graph);
        patch
        // 5. Lock dropped here (end of scope).
    };

    // 6. Emit AFTER lock release so IPC does not block other commands.
    if let Err(e) = app.emit(GRAPH_UPDATED, &patch) {
        log::warn!("[watcher] emit failed: {e}");
    }
}

/// Fallback for the theoretical case where `graph_json` is `None` at dispatch
/// time. The caller (`start_watch_project`) refuses to run without a loaded
/// project, so this path should be unreachable in practice.
fn empty_graph_placeholder() -> GraphJson {
    use std::collections::BTreeMap;

    use crate::types::graph_json::ProjectJson;
    use crate::types::status::Status;

    GraphJson {
        project: ProjectJson {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            node_count: 0,
            module_count: 0,
            fn_count: 0,
            status: Status::Ok,
        },
        clusters: Vec::new(),
        modules: Vec::new(),
        externals: Vec::new(),
        relations: Vec::new(),
        types: Vec::new(),
        errors: Vec::new(),
        issues: Vec::new(),
        detail: BTreeMap::new(),
    }
}

fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Pure, testable re-pipeline + diff step — isolated from Tauri so
/// integration tests can exercise it without an `AppHandle`.
///
/// `prev` is the baseline graph; `parse_dir` is the project's parse
/// directory. Returns `Some(patch)` on success, `None` if the pipeline
/// failed (the watcher swallows transient parse errors).
#[doc(hidden)]
pub fn run_diff_cycle(
    prev: &GraphJson,
    parse_dir: &Path,
    project_name: &str,
    timestamp: u64,
) -> Option<GraphPatchJson> {
    let verified = match load_verified_from_path(parse_dir) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[watcher] pipeline failed: {e}");
            return None;
        }
    };
    let next = serialize_graph(&verified, project_name);
    Some(diff_graph_at(prev, &next, timestamp))
}
