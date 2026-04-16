//! `ail search` — set up or query semantic search.

use std::path::{Path, PathBuf};

use crate::error::CliError;

/// Entry point for `ail search`.
///
/// - `--setup`: check whether the local ONNX model files are present and print
///   status or download instructions.
/// - `<query>`: run BM25 full-text search against the project `.ail.db` and
///   print ranked results. Hybrid (ONNX) ranking via `ail serve` / MCP is the
///   recommended path for AI-assisted workflows.
/// - Neither: print usage guidance.
pub fn run_search(
    cwd: &Path,
    query: Option<&str>,
    budget: usize,
    setup: bool,
    semantic: bool,
    bm25_only: bool,
) -> Result<(), CliError> {
    if setup {
        run_search_setup()
    } else if let Some(q) = query {
        run_search_query(cwd, q, budget, semantic, bm25_only)
    } else {
        Err(CliError::NotImplemented {
            feature: "ail search — provide a query or use --setup",
        })
    }
}

/// Check whether the local ONNX model files are present and print status or
/// download instructions. Does NOT download the model automatically.
fn run_search_setup() -> Result<(), CliError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    let model_dir = Path::new(&home).join(".ail/models/all-MiniLM-L6-v2");
    let model_onnx = model_dir.join("model.onnx");
    let tokenizer_json = model_dir.join("tokenizer.json");

    if model_onnx.exists() && tokenizer_json.exists() {
        println!("Model ready at {}", model_dir.display());
        println!("Run `ail reindex --embeddings` on a migrated project to build the index.");
    } else {
        println!("Model not found at {}", model_dir.display());
        println!();
        println!("To enable semantic search, place the following files there:");
        println!("  {}", model_onnx.display());
        println!("  {}", tokenizer_json.display());
        println!();
        println!("Download from:");
        println!("  https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2");
        println!("  (export model.onnx, download tokenizer.json)");
    }
    Ok(())
}

/// Run search against the project `.ail.db`.
///
/// Without `--semantic`, uses the SQLite FTS5 BM25 index. With `--semantic`
/// (requires the `embeddings` feature), loads persisted vectors from the DB,
/// runs hybrid RRF fusion, and prints provenance metadata per result.
fn run_search_query(
    cwd: &Path,
    query: &str,
    budget: usize,
    semantic: bool,
    bm25_only: bool,
) -> Result<(), CliError> {
    if semantic && bm25_only {
        eprintln!("Warning: --bm25-only overrides --semantic; using BM25 only.");
    }

    let use_semantic = semantic && !bm25_only;

    if use_semantic {
        return run_semantic_search(cwd, query, budget);
    }

    run_bm25_search(cwd, query, budget)
}

/// BM25-only search via SQLite FTS5.
fn run_bm25_search(cwd: &Path, query: &str, budget: usize) -> Result<(), CliError> {
    let db_path = find_db(cwd).ok_or_else(|| CliError::MigrationFailed {
        message: "no .ail.db found in current directory; run 'ail migrate' first".to_string(),
    })?;

    let db = ail_db::SqliteGraph::open(&db_path).map_err(|e| CliError::MigrationFailed {
        message: format!("could not open {}: {e}", db_path.display()),
    })?;

    let results = db
        .search(query, budget)
        .map_err(|e| CliError::MigrationFailed {
            message: format!("search failed: {e}"),
        })?;

    if results.is_empty() {
        println!("No results for {:?}.", query);
    } else {
        println!("Results for {:?} — {} match(es):", query, results.len());
        for (i, r) in results.iter().enumerate() {
            let path_str = if r.path.is_empty() {
                r.intent.clone()
            } else {
                r.path.join(" > ")
            };
            println!(
                "  {}. [{:?}] {} (score: {:.2})",
                i + 1,
                r.pattern,
                path_str,
                r.score
            );
        }
    }

    Ok(())
}

/// Hybrid (BM25 + semantic) search via persisted embeddings and RRF fusion.
#[cfg(feature = "embeddings")]
fn run_semantic_search(cwd: &Path, query: &str, budget: usize) -> Result<(), CliError> {
    use ail_db::EmbeddingModelStatus;
    use ail_search::{
        hybrid_search, EmbeddingIndex, EmbeddingProvider, OnnxEmbeddingProvider, RankingSource,
        DEFAULT_MODEL_NAME, ONNX_DIMENSION,
    };

    let db_path = find_db(cwd).ok_or_else(|| CliError::MigrationFailed {
        message: "no .ail.db found in current directory; run 'ail migrate' first".to_string(),
    })?;

    let db = ail_db::SqliteGraph::open(&db_path).map_err(|e| CliError::MigrationFailed {
        message: format!("could not open {}: {e}", db_path.display()),
    })?;

    // Check full metadata compatibility (model, provider, dimensions, index version).
    let provider_name = DEFAULT_MODEL_NAME.split('/').next().unwrap_or("unknown");
    match db
        .check_embedding_metadata(DEFAULT_MODEL_NAME, provider_name, ONNX_DIMENSION)
        .map_err(|e| CliError::MigrationFailed {
            message: format!("check_embedding_metadata failed: {e}"),
        })? {
        EmbeddingModelStatus::Empty => {
            return Err(CliError::MigrationFailed {
                message: "no embeddings found; run `ail reindex --embeddings` first".to_string(),
            });
        }
        EmbeddingModelStatus::Changed { stored } => {
            return Err(CliError::MigrationFailed {
                message: format!(
                    "embedding metadata mismatch: {stored}; \
                     run `ail reindex --embeddings` to rebuild"
                ),
            });
        }
        EmbeddingModelStatus::Compatible => {}
    }

    // Load persisted vectors and build the in-memory embedding index.
    let vectors = db
        .load_all_embeddings()
        .map_err(|e| CliError::MigrationFailed {
            message: format!("load_all_embeddings failed: {e}"),
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

    println!(
        "Semantic search with model '{}' ({} vectors loaded)",
        provider.name(),
        vectors.len()
    );

    let emb_index = EmbeddingIndex::from_vectors(Box::new(provider), vectors);

    // Run BM25 leg via SQLite FTS5.
    let bm25_raw = db
        .search(query, budget)
        .map_err(|e| CliError::MigrationFailed {
            message: format!("BM25 search failed: {e}"),
        })?;

    // Hybrid RRF fusion.
    let results = hybrid_search(query, &bm25_raw, Some(&emb_index), &db, budget).map_err(|e| {
        CliError::MigrationFailed {
            message: format!("hybrid_search failed: {e}"),
        }
    })?;

    if results.is_empty() {
        println!("No results for {:?}.", query);
    } else {
        println!("Results for {:?} — {} match(es):", query, results.len());
        for (i, r) in results.iter().enumerate() {
            let source = match r.source {
                RankingSource::Bm25Only => "bm25",
                RankingSource::SemanticOnly => "semantic",
                RankingSource::Both => "both",
            };
            let path_str = if r.path.is_empty() {
                r.intent.clone()
            } else {
                r.path.join(" > ")
            };
            println!(
                "  {}. [{:?}] {} (rrf: {:.4}, source: {})",
                i + 1,
                r.pattern,
                path_str,
                r.rrf_score,
                source
            );
        }
    }

    Ok(())
}

/// Without the `embeddings` feature, `--semantic` is not available.
#[cfg(not(feature = "embeddings"))]
fn run_semantic_search(_cwd: &Path, _query: &str, _budget: usize) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        feature: "ail search --semantic requires the 'embeddings' feature; \
                  compile with `cargo build -p ail-cli --features embeddings`",
    })
}

/// Locate the project `.ail.db` file in `root`.
///
/// Checks `project.ail.db` first (conventional name), then scans the directory
/// for any `*.ail.db` file. Mirrors the lookup used by `status.rs` and
/// `reindex.rs`.
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
