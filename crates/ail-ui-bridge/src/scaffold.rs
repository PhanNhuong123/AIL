//! Project scaffolding command — closes finding **N2** from the v4.0
//! acceptance review. Writes a minimal `.ail` skeleton to disk so the
//! Welcome / Quick Create flow can hand a real project path to
//! `load_project`.
//!
//! ## Boundaries
//!
//! - The command body holds no `BridgeState` lock; it is pure file I/O on
//!   caller-supplied paths (no shared mutable state). Adds zero new
//!   `BridgeStateInner` fields (preserves invariant 16.5-I / 16.6-G).
//! - `validate_name` and `render_skeleton` are pure helpers — no I/O, no
//!   panics — and are unit-tested in `tests/scaffold.rs`.
//! - All write paths are absolute joins below `parent_dir`. The command
//!   never escapes the caller-supplied directory and never overwrites an
//!   existing project (returns `InvalidInput` instead).

#![cfg(feature = "tauri-commands")]

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::BridgeError;
use crate::types::scaffold::{ProjectScaffoldRequest, ProjectScaffoldResult};

// ---------------------------------------------------------------------------
// Pure helpers — no I/O, no locks
// ---------------------------------------------------------------------------

/// Validate a project / node name.
///
/// Allowed: ASCII letters, digits, and `_`. Must start with a letter and be
/// 1..=64 characters long. Empty, leading-digit, or symbol-bearing names
/// return `Err(reason)`.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name is empty".to_string());
    }
    if name.len() > 64 {
        return Err("name exceeds 64 characters".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().expect("non-empty checked above");
    if !first.is_ascii_alphabetic() {
        return Err("name must start with an ASCII letter".to_string());
    }
    for c in std::iter::once(first).chain(chars) {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("name contains invalid character {c:?}"));
        }
    }
    Ok(())
}

/// Validate the Quick-Create kind. Returns the canonical lowercase form on
/// success.
pub fn validate_kind(kind: &str) -> Result<&'static str, String> {
    match kind {
        "module" => Ok("module"),
        "function" => Ok("function"),
        "rule" => Ok("rule"),
        "test" => Ok("test"),
        other => Err(format!(
            "unknown kind '{other}' (expected module|function|rule|test)"
        )),
    }
}

/// Render the skeleton `.ail` file body for `kind` and `name`. Pure; the
/// `description` is included verbatim in a leading comment when non-empty.
///
/// Output is deterministic — tests in `tests/scaffold.rs` lock the exact
/// shape so consumers can rely on it.
pub fn render_skeleton(kind: &str, name: &str, description: &str) -> String {
    let mut out = String::new();
    if !description.trim().is_empty() {
        out.push_str("// ");
        out.push_str(description.trim());
        out.push('\n');
        out.push('\n');
    }
    match kind {
        "module" => {
            out.push_str(&format!(
                "module {name} {{\n  // Define functions, rules, and tests inside this module.\n}}\n"
            ));
        }
        "function" => {
            out.push_str(&format!(
                "module {name} {{\n  function {name}() {{\n    // implementation\n  }}\n}}\n"
            ));
        }
        "rule" => {
            out.push_str(&format!(
                "module {name} {{\n  rule {name} {{\n    // postcondition: <expr>\n  }}\n}}\n"
            ));
        }
        "test" => {
            out.push_str(&format!(
                "module {name} {{\n  test {name} {{\n    // assertions\n  }}\n}}\n"
            ));
        }
        _ => unreachable!("validate_kind should reject unknown kinds"),
    }
    out
}

/// Render the minimal `ail.config.toml` body.
pub fn render_config(name: &str) -> String {
    format!("[project]\nname = \"{name}\"\nversion = \"0.1.0\"\n")
}

// ---------------------------------------------------------------------------
// Tauri command (no `BridgeState` lock — pure file I/O on caller paths)
// ---------------------------------------------------------------------------

/// Write a minimal AIL project skeleton at `<parent_dir>/<name>`.
///
/// Returns `ProjectScaffoldResult { project_dir, ail_file }` on success.
/// Returns `BridgeError::InvalidInput` if the name/kind is invalid, the
/// parent does not exist, or the target project directory already exists.
#[tauri::command]
pub fn scaffold_project(
    request: ProjectScaffoldRequest,
) -> Result<ProjectScaffoldResult, BridgeError> {
    let kind =
        validate_kind(&request.kind).map_err(|reason| BridgeError::InvalidInput { reason })?;
    validate_name(&request.name).map_err(|reason| BridgeError::InvalidInput { reason })?;

    let parent = PathBuf::from(&request.parent_dir);
    if !parent.is_dir() {
        return Err(BridgeError::InvalidInput {
            reason: format!("parent directory does not exist: {}", parent.display()),
        });
    }

    let project_dir = parent.join(&request.name);
    if project_dir.exists() {
        return Err(BridgeError::InvalidInput {
            reason: format!(
                "target project directory already exists: {}",
                project_dir.display()
            ),
        });
    }

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| BridgeError::InvalidInput {
        reason: format!("failed to create project directory: {e}"),
    })?;

    let ail_file = src_dir.join(format!("{}.ail", request.name));
    let body = render_skeleton(kind, &request.name, &request.description);
    write_file(&ail_file, &body)?;

    let config_file = project_dir.join("ail.config.toml");
    let config_body = render_config(&request.name);
    write_file(&config_file, &config_body)?;

    Ok(ProjectScaffoldResult {
        project_dir: project_dir.to_string_lossy().into_owned(),
        ail_file: ail_file.to_string_lossy().into_owned(),
    })
}

fn write_file(path: &Path, contents: &str) -> Result<(), BridgeError> {
    fs::write(path, contents).map_err(|e| BridgeError::InvalidInput {
        reason: format!("failed to write {}: {e}", path.display()),
    })
}
