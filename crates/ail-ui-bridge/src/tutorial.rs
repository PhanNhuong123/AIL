//! Tutorial path resolver — closes finding **N1.b** (Welcome "Try the
//! tutorial" button) from the v4.0 acceptance review.
//!
//! Returns the absolute path of the bundled `examples/wallet_service`
//! tutorial project. The frontend hands the result to `loadProject` so the
//! Welcome modal can launch a one-click demo.
//!
//! ## Resolution rules
//!
//! - **Dev mode** (`AIL_DEV=1`): walk up from `CARGO_MANIFEST_DIR` until a
//!   `examples/wallet_service` sibling is found. The repo layout is
//!   `crates/ail-ui-bridge/` → workspace root → `examples/wallet_service`,
//!   so two `parent()` hops suffice. Falls back to `cwd()/examples/...`
//!   when the manifest dir env var is not set (test contexts).
//! - **Bundle mode**: `app.path().resource_dir().join("examples/wallet_service")`.
//!   Tauri ships the example as a `bundle.resources` entry — see
//!   `ide/src-tauri/tauri.conf.json`.
//!
//! Adds zero new `BridgeStateInner` fields (preserves invariant
//! 16.5-I / 16.6-G). Holds no lock; no I/O beyond `Path::exists` and
//! `env::var`.

#![cfg(feature = "tauri-commands")]

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};

use crate::errors::BridgeError;
use crate::sidecar::parse_ail_dev_mode;

const TUTORIAL_RELATIVE: &str = "examples/wallet_service";

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Walk up `start` looking for `examples/wallet_service`. Returns the first
/// match. Pure — only `Path::exists` syscalls.
pub fn find_tutorial_in_workspace(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join(TUTORIAL_RELATIVE);
        if candidate.is_dir() {
            return Some(candidate);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Resolve the tutorial path in dev mode. Tries `CARGO_MANIFEST_DIR` first,
/// then `cwd()`. Returns `None` if the example is not present (e.g. an
/// extracted release tarball without sources).
pub fn resolve_tutorial_path_dev() -> Option<PathBuf> {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Some(p) = find_tutorial_in_workspace(Path::new(&manifest_dir)) {
            return Some(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(p) = find_tutorial_in_workspace(&cwd) {
            return Some(p);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tauri command
// ---------------------------------------------------------------------------

/// Return the absolute path of the bundled tutorial project.
///
/// Errors with `InvalidInput` when the tutorial cannot be located in the
/// current execution environment (e.g. a stripped release build that
/// dropped `examples/`).
#[tauri::command]
pub fn get_tutorial_path<R: Runtime>(app: AppHandle<R>) -> Result<String, BridgeError> {
    if parse_ail_dev_mode() {
        if let Some(path) = resolve_tutorial_path_dev() {
            return Ok(path.to_string_lossy().into_owned());
        }
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| BridgeError::InvalidInput {
            reason: format!("failed to resolve resource dir: {e}"),
        })?;
    let bundled = resource_dir.join(TUTORIAL_RELATIVE);
    if bundled.is_dir() {
        return Ok(bundled.to_string_lossy().into_owned());
    }

    // Last-resort fallback: try the dev resolver even when AIL_DEV is unset.
    // This covers `cargo run` from the repo without the env flag.
    if let Some(path) = resolve_tutorial_path_dev() {
        return Ok(path.to_string_lossy().into_owned());
    }

    Err(BridgeError::InvalidInput {
        reason: format!(
            "tutorial not found in bundle ({}) or workspace",
            bundled.display()
        ),
    })
}
