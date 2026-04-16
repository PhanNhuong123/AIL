//! `ail reindex` — rebuild the embedding index for a `.ail.db` project.

use std::path::{Path, PathBuf};

use ail_db::SqliteGraph;

use crate::error::CliError;

/// Entry point for `ail reindex`.
///
/// Locates the `.ail.db` file in `cwd`, then:
/// - Without `--embeddings`: clears the stored embedding vectors and prints
///   instructions to run `ail reindex --embeddings` after model setup.
/// - With `--embeddings`: embeds all project nodes using the local ONNX model
///   and writes the vectors to the DB. Requires the `embeddings` feature and
///   model files at `~/.ail/models/all-MiniLM-L6-v2/`.
pub fn run_reindex(cwd: &Path, embeddings: bool) -> Result<(), CliError> {
    let db_path = find_db(cwd).ok_or_else(|| CliError::MigrationFailed {
        message: "no .ail.db found in current directory; run 'ail migrate' first".to_string(),
    })?;

    if embeddings {
        do_reindex_embeddings(&db_path)
    } else {
        let db = SqliteGraph::open(&db_path).map_err(|e| CliError::MigrationFailed {
            message: format!("could not open {}: {e}", db_path.display()),
        })?;

        let count_before = db.embedding_count().unwrap_or(0);
        db.clear_embeddings()
            .map_err(|e| CliError::MigrationFailed {
                message: format!("clear_embeddings failed: {e}"),
            })?;

        println!(
            "Cleared {count_before} embedding vector(s) from {}.",
            db_path.display()
        );
        println!(
            "Run `ail search --setup` to verify model files, then `ail reindex --embeddings` to rebuild."
        );
        Ok(())
    }
}

/// Embed all project nodes using the local ONNX model and persist to the DB.
///
/// Clears existing vectors first so the mixed-index guard allows any model.
/// Each node's embedding text is produced by `ail_search::node_embedding_text`.
#[cfg(feature = "embeddings")]
fn do_reindex_embeddings(db_path: &Path) -> Result<(), CliError> {
    use ail_graph::GraphBackend;
    use ail_search::{node_embedding_text, EmbeddingProvider, OnnxEmbeddingProvider};

    let db = SqliteGraph::open(db_path).map_err(|e| CliError::MigrationFailed {
        message: format!("could not open {}: {e}", db_path.display()),
    })?;

    let model_dir =
        OnnxEmbeddingProvider::ensure_model().map_err(|e| CliError::MigrationFailed {
            message: format!(
                "model not found: {e}\nRun `ail search --setup` to get setup instructions."
            ),
        })?;

    let provider =
        OnnxEmbeddingProvider::new(&model_dir).map_err(|e| CliError::MigrationFailed {
            message: format!("failed to load embedding model: {e}"),
        })?;

    let model_name = provider.name().to_string();
    let node_ids = db.all_node_ids().map_err(|e| CliError::MigrationFailed {
        message: format!("could not read node list: {e}"),
    })?;

    println!(
        "Embedding {} node(s) with model '{}'...",
        node_ids.len(),
        model_name
    );

    // Clear existing vectors so save_embeddings_bulk passes the mixed-index guard.
    db.clear_embeddings()
        .map_err(|e| CliError::MigrationFailed {
            message: format!("clear_embeddings failed: {e}"),
        })?;

    let mut pairs = Vec::with_capacity(node_ids.len());
    for id in node_ids {
        if let Some(node) = db.get_node(id).map_err(|e| CliError::MigrationFailed {
            message: format!("get_node failed: {e}"),
        })? {
            let text = node_embedding_text(&node);
            let vec = provider
                .embed(&text)
                .map_err(|e| CliError::MigrationFailed {
                    message: format!("embedding failed for node {id}: {e}"),
                })?;
            pairs.push((id, vec));
        }
    }

    db.save_embeddings_bulk(&pairs, &model_name)
        .map_err(|e| CliError::MigrationFailed {
            message: format!("save_embeddings_bulk failed: {e}"),
        })?;

    println!("Done. Indexed {} node(s).", pairs.len());
    Ok(())
}

/// Without the `embeddings` feature, `--embeddings` is not available.
#[cfg(not(feature = "embeddings"))]
fn do_reindex_embeddings(_db_path: &Path) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        feature: "ail reindex --embeddings — compile ail-cli with the 'embeddings' feature to enable ONNX inference",
    })
}

/// Locate an `.ail.db` file in `root`.
///
/// Checks `project.ail.db` first (conventional name), then scans the directory
/// for any `*.ail.db` file. Mirrors the lookup in `commands/status.rs`.
fn find_db(root: &Path) -> Option<PathBuf> {
    let conventional = root.join("project.ail.db");
    if conventional.exists() {
        return Some(conventional);
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("db")
            && path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.ends_with(".ail"))
                .unwrap_or(false)
        {
            return Some(path);
        }
    }
    None
}
