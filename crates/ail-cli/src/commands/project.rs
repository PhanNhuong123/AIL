//! Project-root helpers shared by `verify`, `build`, and `context`.
//!
//! Resolves which graph backend to load for read operations based on
//! `ail.config.toml [database] backend = "auto" | "sqlite" | "filesystem"`
//! plus presence of a `.ail.db` file next to the config.

use std::fs;
use std::path::{Path, PathBuf};

use ail_db::SqliteGraph;
use ail_graph::AilGraph;
use ail_text::parse_directory;

use crate::commands::migrate::rebuild_from_sqlite;
use crate::error::CliError;

/// Resolved backend choice for a project root.
#[derive(Debug, Clone)]
pub enum ProjectBackend {
    Filesystem { src_dir: PathBuf },
    Sqlite { db_path: PathBuf },
}

/// Resolve the backend to use for `project_root`.
///
/// Order of precedence:
/// 1. `override_db`: caller-supplied `--from-db` path forces SQLite.
/// 2. `ail.config.toml [database] backend = "sqlite" | "filesystem"` forces that backend.
/// 3. `backend = "auto"` (or missing): use SQLite when `project.ail.db` exists next to
///    the config, otherwise filesystem.
/// 4. No config file: default to filesystem.
pub fn resolve_backend(
    project_root: &Path,
    override_db: Option<&Path>,
) -> Result<ProjectBackend, CliError> {
    if let Some(db) = override_db {
        return Ok(ProjectBackend::Sqlite {
            db_path: db.to_path_buf(),
        });
    }

    let default_db = project_root.join("project.ail.db");
    let default_src = project_root.join("src");
    let src_dir = if default_src.is_dir() {
        default_src
    } else {
        project_root.to_path_buf()
    };

    match read_backend_setting(project_root) {
        BackendSetting::Sqlite => {
            if !default_db.exists() {
                return Err(CliError::Pipeline {
                    errors: format!(
                        "config requests SQLite backend but {} does not exist",
                        default_db.display()
                    ),
                });
            }
            Ok(ProjectBackend::Sqlite {
                db_path: default_db,
            })
        }
        BackendSetting::Filesystem => Ok(ProjectBackend::Filesystem { src_dir }),
        BackendSetting::Auto | BackendSetting::Unspecified => {
            if default_db.exists() {
                Ok(ProjectBackend::Sqlite {
                    db_path: default_db,
                })
            } else {
                Ok(ProjectBackend::Filesystem { src_dir })
            }
        }
    }
}

/// Load an `AilGraph` using the resolved backend.
///
/// For SQLite, opens the DB and calls `rebuild_from_sqlite` so callers get a
/// shape-equivalent `AilGraph` suitable for the existing validate/type/verify
/// pipeline.
pub fn load_graph(backend: &ProjectBackend) -> Result<AilGraph, CliError> {
    match backend {
        ProjectBackend::Filesystem { src_dir } => {
            parse_directory(src_dir).map_err(|e| CliError::Pipeline {
                errors: e.to_string(),
            })
        }
        ProjectBackend::Sqlite { db_path } => {
            let db = SqliteGraph::open(db_path).map_err(|e| CliError::Pipeline {
                errors: format!("open {}: {e}", db_path.display()),
            })?;
            rebuild_from_sqlite(&db)
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum BackendSetting {
    Auto,
    Sqlite,
    Filesystem,
    Unspecified,
}

fn read_backend_setting(project_root: &Path) -> BackendSetting {
    let Ok(text) = fs::read_to_string(project_root.join("ail.config.toml")) else {
        return BackendSetting::Unspecified;
    };

    let mut in_database_section = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_database_section = line == "[database]";
            continue;
        }
        if !in_database_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix("backend") {
            let value = rest
                .trim_start_matches(|c: char| c == '=' || c.is_whitespace())
                .trim();
            let value = value.split('#').next().unwrap_or(value).trim();
            let value = value.trim_matches(|c: char| c == '"' || c == '\'');
            return match value {
                "auto" => BackendSetting::Auto,
                "sqlite" => BackendSetting::Sqlite,
                "filesystem" => BackendSetting::Filesystem,
                _ => BackendSetting::Unspecified,
            };
        }
    }
    BackendSetting::Unspecified
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(root: &Path, body: &str) {
        fs::write(root.join("ail.config.toml"), body).unwrap();
    }

    #[test]
    fn backend_auto_picks_sqlite_when_db_exists() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), "[database]\nbackend = \"auto\"\n");
        fs::write(tmp.path().join("project.ail.db"), b"").unwrap();

        let b = resolve_backend(tmp.path(), None).unwrap();
        assert!(matches!(b, ProjectBackend::Sqlite { .. }));
    }

    #[test]
    fn backend_auto_falls_back_to_filesystem() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), "[database]\nbackend = \"auto\"\n");

        let b = resolve_backend(tmp.path(), None).unwrap();
        assert!(matches!(b, ProjectBackend::Filesystem { .. }));
    }

    #[test]
    fn from_db_override_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let explicit = tmp.path().join("other.ail.db");
        let b = resolve_backend(tmp.path(), Some(&explicit)).unwrap();
        match b {
            ProjectBackend::Sqlite { db_path } => assert_eq!(db_path, explicit),
            _ => panic!("expected sqlite override"),
        }
    }

    #[test]
    fn backend_sqlite_requires_db_present() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), "[database]\nbackend = \"sqlite\"\n");

        let err = resolve_backend(tmp.path(), None).unwrap_err();
        assert!(err.to_string().contains("SQLite"));
    }

    #[test]
    fn unspecified_config_defaults_to_filesystem_without_db() {
        let tmp = tempfile::tempdir().unwrap();
        let b = resolve_backend(tmp.path(), None).unwrap();
        assert!(matches!(b, ProjectBackend::Filesystem { .. }));
    }
}
