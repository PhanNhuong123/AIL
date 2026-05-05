//! Sidecar layout I/O.
//!
//! Stores per-project node coordinates at `<project>/.ail/layout.json`. The
//! file is read on project load and merged on every `save_flowchart` call.
//! Writes go through [`crate::save_registry`] so the watcher's echo path
//! stays consistent — even though the layout file itself is not a `.ail`
//! source and would not trigger the watcher, recording the path keeps the
//! audit trail uniform with future `.ail` writes.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::save_registry::{save_registry, SaveContext, SaveSource};
use crate::types::layout::{LayoutEntry, ProjectLayout};

/// Subdirectory under the project root that holds layout + future sidecar
/// state. Created on first write.
pub const SIDECAR_DIR: &str = ".ail";
pub const LAYOUT_FILE: &str = "layout.json";

/// I/O errors that can surface from the layout module. Distinct from
/// `BridgeError` so callers can choose to surface a specific failure or
/// degrade gracefully.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("layout io: {0}")]
    Io(#[from] io::Error),
    #[error("layout parse: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("layout schema version {got} unsupported (expected ≤ {max})")]
    UnsupportedVersion { got: u32, max: u32 },
}

/// Build the absolute layout path for a project root.
pub fn layout_path(project_root: &Path) -> PathBuf {
    project_root.join(SIDECAR_DIR).join(LAYOUT_FILE)
}

/// Load the project's layout. Missing file → `Ok(ProjectLayout::default())`
/// so a fresh project does not error.
pub fn load_layout(project_root: &Path) -> Result<ProjectLayout, LayoutError> {
    let path = layout_path(project_root);
    if !path.is_file() {
        return Ok(ProjectLayout::default());
    }
    let raw = fs::read_to_string(&path)?;
    let layout: ProjectLayout = serde_json::from_str(&raw)?;
    if layout.version > ProjectLayout::CURRENT_VERSION {
        return Err(LayoutError::UnsupportedVersion {
            got: layout.version,
            max: ProjectLayout::CURRENT_VERSION,
        });
    }
    Ok(layout)
}

/// Atomically replace the layout file with `layout`'s serialised form.
///
/// The save is recorded in [`save_registry`] under [`SaveSource::Ui`] so the
/// echo-suppression mechanism remains symmetric with future `.ail` writes.
/// `session_id` lets callers tie multiple saves together (e.g. a single drag
/// gesture that issues several debounced writes).
pub fn save_layout(
    project_root: &Path,
    layout: &ProjectLayout,
    session_id: impl Into<String>,
) -> Result<(), LayoutError> {
    let path = layout_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(layout)?;
    write_atomic(&path, json.as_bytes())?;
    save_registry().record(
        path.clone(),
        SaveContext {
            source: SaveSource::Ui,
            session_id: session_id.into(),
        },
    );
    Ok(())
}

/// Merge `updates` into the on-disk layout (creating the file if absent) and
/// persist the result. Convenience helper used by `save_flowchart`.
pub fn merge_and_save(
    project_root: &Path,
    updates: HashMap<String, LayoutEntry>,
    session_id: impl Into<String>,
) -> Result<ProjectLayout, LayoutError> {
    let mut current = load_layout(project_root).unwrap_or_default();
    current.merge(updates);
    current.version = ProjectLayout::CURRENT_VERSION;
    save_layout(project_root, &current, session_id)?;
    Ok(current)
}

/// Write `bytes` to `path` atomically (write to `path.tmp`, then rename).
/// Atomic rename guarantees readers never see partial JSON.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(x: f64, y: f64) -> LayoutEntry {
        LayoutEntry { x, y }
    }

    #[test]
    fn test_load_layout_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let layout = load_layout(dir.path()).expect("missing → default");
        assert!(layout.nodes.is_empty());
        assert_eq!(layout.version, ProjectLayout::CURRENT_VERSION);
    }

    #[test]
    fn test_save_then_load_roundtrip() {
        let dir = tempdir().unwrap();
        let mut layout = ProjectLayout::new();
        layout.nodes.insert("foo".to_string(), entry(10.0, 20.0));
        save_layout(dir.path(), &layout, "s1").unwrap();
        let loaded = load_layout(dir.path()).unwrap();
        assert_eq!(loaded, layout);
    }

    #[test]
    fn test_save_creates_sidecar_directory() {
        let dir = tempdir().unwrap();
        save_layout(dir.path(), &ProjectLayout::new(), "s1").unwrap();
        assert!(dir.path().join(SIDECAR_DIR).is_dir());
        assert!(dir.path().join(SIDECAR_DIR).join(LAYOUT_FILE).is_file());
    }

    #[test]
    fn test_save_records_in_registry() {
        let dir = tempdir().unwrap();
        save_layout(dir.path(), &ProjectLayout::new(), "session-xyz").unwrap();
        let path = layout_path(dir.path());
        let session = save_registry()
            .last_session(&path)
            .expect("layout save must register");
        assert_eq!(session.source, SaveSource::Ui);
        assert_eq!(session.session_id, "session-xyz");
    }

    #[test]
    fn test_merge_and_save_creates_when_missing() {
        let dir = tempdir().unwrap();
        let mut updates = HashMap::new();
        updates.insert("a".to_string(), entry(1.0, 2.0));
        let result = merge_and_save(dir.path(), updates, "s1").unwrap();
        assert_eq!(result.nodes["a"], entry(1.0, 2.0));
        let loaded = load_layout(dir.path()).unwrap();
        assert_eq!(loaded.nodes["a"], entry(1.0, 2.0));
    }

    #[test]
    fn test_merge_and_save_preserves_existing_entries() {
        let dir = tempdir().unwrap();
        let initial = ProjectLayout::new()
            .with_node("a", entry(1.0, 1.0))
            .with_node("b", entry(2.0, 2.0));
        save_layout(dir.path(), &initial, "init").unwrap();

        let mut updates = HashMap::new();
        updates.insert("b".to_string(), entry(99.0, 99.0));
        updates.insert("c".to_string(), entry(3.0, 3.0));
        let result = merge_and_save(dir.path(), updates, "second").unwrap();

        assert_eq!(result.nodes["a"], entry(1.0, 1.0));
        assert_eq!(result.nodes["b"], entry(99.0, 99.0));
        assert_eq!(result.nodes["c"], entry(3.0, 3.0));
    }

    #[test]
    fn test_load_layout_rejects_unsupported_version() {
        let dir = tempdir().unwrap();
        let path = layout_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"version\": 99, \"nodes\": {}}").unwrap();
        let err = load_layout(dir.path()).unwrap_err();
        assert!(matches!(err, LayoutError::UnsupportedVersion { got: 99, .. }));
    }

    #[test]
    fn test_load_layout_rejects_invalid_json() {
        let dir = tempdir().unwrap();
        let path = layout_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ this is not valid json").unwrap();
        let err = load_layout(dir.path()).unwrap_err();
        assert!(matches!(err, LayoutError::Parse(_)));
    }

    #[test]
    fn test_save_atomic_no_partial_writes_visible() {
        // `write_atomic` writes to `<path>.tmp` then renames. After save the
        // tmp file must be gone (rename, not copy).
        let dir = tempdir().unwrap();
        save_layout(dir.path(), &ProjectLayout::new(), "s").unwrap();
        let final_path = layout_path(dir.path());
        let tmp_path = final_path.with_extension("json.tmp");
        assert!(final_path.is_file());
        assert!(!tmp_path.exists(), "tmp must be renamed away after save");
    }

    #[test]
    fn test_layout_path_under_sidecar_dir() {
        let p = layout_path(Path::new("/proj"));
        assert!(p.ends_with(format!("{SIDECAR_DIR}/{LAYOUT_FILE}")));
    }

    #[test]
    fn test_orphan_entries_survive_merge() {
        // A node id that no longer exists in the graph (e.g. removed by a
        // refactor) must NOT be silently dropped — the next save preserves
        // it so undo/restore via Git remains possible.
        let dir = tempdir().unwrap();
        let initial = ProjectLayout::new()
            .with_node("alive", entry(1.0, 1.0))
            .with_node("orphan", entry(99.0, 99.0));
        save_layout(dir.path(), &initial, "init").unwrap();

        let mut updates = HashMap::new();
        updates.insert("alive".to_string(), entry(2.0, 2.0));
        let result = merge_and_save(dir.path(), updates, "drag").unwrap();

        assert_eq!(result.nodes["alive"], entry(2.0, 2.0));
        assert_eq!(
            result.nodes["orphan"],
            entry(99.0, 99.0),
            "orphan entries must not be silently dropped"
        );
    }

    #[test]
    fn test_empty_updates_is_idempotent() {
        let dir = tempdir().unwrap();
        let initial = ProjectLayout::new().with_node("a", entry(1.0, 1.0));
        save_layout(dir.path(), &initial, "init").unwrap();

        let result = merge_and_save(dir.path(), HashMap::new(), "noop").unwrap();
        assert_eq!(result, initial);
    }

    #[test]
    fn test_save_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let v1 = ProjectLayout::new().with_node("a", entry(1.0, 1.0));
        save_layout(dir.path(), &v1, "v1").unwrap();
        let v2 = ProjectLayout::new().with_node("b", entry(2.0, 2.0));
        save_layout(dir.path(), &v2, "v2").unwrap();
        let loaded = load_layout(dir.path()).unwrap();
        // v2 fully replaces v1 (no merge at the save_layout layer).
        assert!(loaded.nodes.contains_key("b"));
        assert!(!loaded.nodes.contains_key("a"));
    }

    #[test]
    fn test_layout_entry_float_precision_roundtrips() {
        // f32 has ~7 decimal digits. Verify precise positions survive a
        // roundtrip — drag UX is sensitive to single-pixel drift.
        let dir = tempdir().unwrap();
        let layout = ProjectLayout::new()
            .with_node("p", entry(123.456, -78.901))
            .with_node("q", entry(0.0001, 9999.999));
        save_layout(dir.path(), &layout, "s").unwrap();
        let back = load_layout(dir.path()).unwrap();
        assert_eq!(back, layout);
    }

    #[test]
    fn test_deep_node_id_paths_round_trip() {
        // Path-like ids are dotted; verify deep nesting survives JSON.
        let dir = tempdir().unwrap();
        let id = "wallet_service.src.billing.transfer_money.s_validate.guard";
        let layout = ProjectLayout::new().with_node(id, entry(50.0, 50.0));
        save_layout(dir.path(), &layout, "s").unwrap();
        let back = load_layout(dir.path()).unwrap();
        assert_eq!(back.nodes[id], entry(50.0, 50.0));
    }
}
