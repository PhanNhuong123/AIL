use std::fs;
use std::path::Path;

use ail_contract::verify;
use ail_graph::validate_graph;
use ail_text::parse_directory;
use ail_types::type_check;

use ail_contract::VerifiedGraph;

use crate::errors::BridgeError;

/// Run the full 4-stage AIL pipeline over a project directory.
///
/// Stages: `parse_directory → validate_graph → type_check → verify`.
/// Each stage error is wrapped into `BridgeError::PipelineError` with the
/// stage name and a joined error string.
///
/// Returns `Err(BridgeError::ProjectNotFound)` if `path` is not a directory.
pub fn load_verified_from_path(path: &Path) -> Result<VerifiedGraph, BridgeError> {
    if !path.is_dir() {
        return Err(BridgeError::ProjectNotFound {
            path: path.display().to_string(),
        });
    }

    // ── 1. Parse ──────────────────────────────────────────────────────────────
    let graph = parse_directory(path).map_err(|e| BridgeError::PipelineError {
        stage: "parse".to_string(),
        detail: e.to_string(),
    })?;

    // ── 2. Validate ───────────────────────────────────────────────────────────
    let valid = validate_graph(graph).map_err(|errs| BridgeError::PipelineError {
        stage: "validate".to_string(),
        detail: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    })?;

    // ── 3. Type-check (packets computed internally by type_check) ─────────────
    let typed = type_check(valid, &[]).map_err(|errs| BridgeError::PipelineError {
        stage: "type_check".to_string(),
        detail: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    })?;

    // ── 4. Contract verification ───────────────────────────────────────────────
    verify(typed).map_err(|errs| BridgeError::PipelineError {
        stage: "verify".to_string(),
        detail: errs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    })
}

/// Read `project.name` from `ail.config.toml` in `project_dir`.
///
/// Uses a tolerant hand-written scan — does not pull in a TOML parser
/// dependency. Falls back to the directory basename on any failure.
pub fn read_project_name(project_dir: &Path) -> String {
    let config_path = project_dir.join("ail.config.toml");
    if let Ok(contents) = fs::read_to_string(&config_path) {
        // Scan for `name = "..."` under `[project]`
        let mut in_project = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" {
                in_project = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_project = false;
                continue;
            }
            if in_project && trimmed.starts_with("name") {
                if let Some(eq_pos) = trimmed.find('=') {
                    let value = trimmed[eq_pos + 1..].trim().trim_matches('"').to_string();
                    if !value.is_empty() {
                        return value;
                    }
                }
            }
        }
    }

    // Fallback: directory basename.
    project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string()
}
