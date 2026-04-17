//! `ail coverage` — compute or display semantic coverage for project nodes.
//!
//! Three modes:
//! - `--node <name>`: compute/display coverage for one node (cache-aware).
//! - `--all`: summarise coverage across all non-leaf nodes (cache-aware).
//! - `--warm-cache`: recompute all non-leaf nodes unconditionally, persist results.
//!
//! All modes require the SQLite backend.  The `embeddings` feature must be
//! enabled for actual computation; the disabled path prints a clear notice.

use std::path::{Path, PathBuf};

#[cfg(feature = "embeddings")]
use std::time::Instant;

use ail_db::SqliteGraph;
use ail_graph::cic::CoverageConfig;

#[cfg(feature = "embeddings")]
use ail_graph::cic::{CoverageInfo, CoverageStatus};

#[cfg(feature = "embeddings")]
use ail_graph::graph::GraphBackend;

#[cfg(feature = "embeddings")]
use ail_graph::types::NodeId;

use crate::commands::project::{resolve_backend, ProjectBackend};
use crate::error::CliError;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Entry point for `ail coverage`.
pub fn run_coverage(
    cwd: &Path,
    node: Option<String>,
    all: bool,
    warm_cache: bool,
    from_db: Option<PathBuf>,
) -> Result<(), CliError> {
    // ── 1. Validate mode selection ────────────────────────────────────────────
    let mode_count = node.is_some() as u8 + all as u8 + warm_cache as u8;
    if mode_count == 0 {
        return Err(CliError::Pipeline {
            errors: "pass one of --node <NAME>, --all, or --warm-cache".to_owned(),
        });
    }
    if mode_count > 1 {
        return Err(CliError::Pipeline {
            errors: "--node, --all, and --warm-cache are mutually exclusive".to_owned(),
        });
    }

    // ── 2. Resolve and open the SQLite backend ────────────────────────────────
    let backend = resolve_backend(cwd, from_db.as_deref())?;
    let db_path = match backend {
        ProjectBackend::Sqlite { db_path } => db_path,
        ProjectBackend::Filesystem { .. } => {
            return Err(CliError::Pipeline {
                errors: "coverage requires a SQLite project — run \
                    `ail migrate --from src/ --to project.ail.db` first, \
                    or pass --from-db <path>."
                    .to_owned(),
            });
        }
    };

    let db = SqliteGraph::open(&db_path).map_err(|e| CliError::Pipeline {
        errors: format!("open {}: {e}", db_path.display()),
    })?;

    // ── 3. Read [coverage] config ─────────────────────────────────────────────
    let cfg = read_coverage_config(cwd);

    if !cfg.enabled {
        println!("coverage disabled via [coverage].enabled=false");
        return Ok(());
    }

    let config_hash = cfg.config_hash();

    // ── 4. Dispatch to sub-commands ───────────────────────────────────────────
    dispatch(db, cfg, config_hash, node, all, warm_cache)
}

// ─── Feature-gated dispatch ───────────────────────────────────────────────────

/// Route to the right sub-command.  The feature gate lives here so that the
/// non-embeddings build still compiles all shared helpers.
#[cfg(feature = "embeddings")]
fn dispatch(
    db: SqliteGraph,
    cfg: CoverageConfig,
    config_hash: String,
    node: Option<String>,
    all: bool,
    warm_cache: bool,
) -> Result<(), CliError> {
    use ail_search::OnnxEmbeddingProvider;

    // Initialise the ONNX embedding provider.
    let provider_result: Result<OnnxEmbeddingProvider, String> =
        OnnxEmbeddingProvider::ensure_model()
            .map_err(|e| e.to_string())
            .and_then(|dir| OnnxEmbeddingProvider::new(&dir).map_err(|e| e.to_string()));

    if let Some(name_or_id) = node {
        match provider_result {
            Ok(provider) => cmd_node(&db, &provider, &cfg, &config_hash, &name_or_id),
            Err(e) => {
                eprintln!(
                    "Coverage unavailable: embedding provider not initialized. \
                     Run `ail reindex --embeddings` first.\nDetail: {e}"
                );
                std::process::exit(1);
            }
        }
    } else if all {
        match provider_result {
            Ok(provider) => cmd_all(&db, &provider, &cfg, &config_hash),
            Err(e) => {
                // Count eligible nodes and report Unavailable for each.
                let total = count_non_leaf_nodes(&db);
                println!("Unavailable: {e}");
                println!("Coverage summary ({total} nodes): Unavailable {total}");
                Ok(())
            }
        }
    } else {
        debug_assert!(warm_cache, "invariant: one of node/all/warm_cache is set");
        match provider_result {
            Ok(provider) => cmd_warm_cache(&db, &provider, &cfg, &config_hash),
            Err(e) => {
                let total = count_non_leaf_nodes(&db);
                println!("Unavailable: {e}");
                println!("Warmed 0 entries (provider unavailable; {total} nodes skipped).");
                Ok(())
            }
        }
    }
}

#[cfg(not(feature = "embeddings"))]
fn dispatch(
    _db: SqliteGraph,
    _cfg: CoverageConfig,
    _config_hash: String,
    _node: Option<String>,
    _all: bool,
    _warm_cache: bool,
) -> Result<(), CliError> {
    eprintln!(
        "Coverage requires the `embeddings` feature. \
         Rebuild with `cargo install ail-cli --features embeddings` \
         or run `cargo run --features embeddings -- coverage ...`"
    );
    std::process::exit(1);
}

// ─── Sub-commands ─────────────────────────────────────────────────────────────

/// Display coverage for a single node by name or id string.
#[cfg(feature = "embeddings")]
fn cmd_node(
    db: &SqliteGraph,
    provider: &dyn ail_search::EmbeddingProvider,
    cfg: &CoverageConfig,
    config_hash: &str,
    name_or_id: &str,
) -> Result<(), CliError> {
    use ail_coverage::compute_coverage;

    let node_id = resolve_node_id(db, name_or_id)?;
    let id_str = node_id.to_string();

    // Try the cache first.
    let (info, cached) =
        match db
            .load_coverage(&id_str, config_hash)
            .map_err(|e| CliError::Pipeline {
                errors: format!("load_coverage: {e}"),
            })? {
            Some(info) => (info, true),
            None => {
                // Cache miss — compute.
                let result =
                    compute_coverage(db, provider, node_id, &cfg.extra_concepts).map_err(|e| {
                        CliError::Pipeline {
                            errors: format!("compute_coverage: {e}"),
                        }
                    })?;
                let info = result.into_info(cfg, config_hash.to_owned());
                db.save_coverage(&id_str, &info)
                    .map_err(|e| CliError::Pipeline {
                        errors: format!("save_coverage: {e}"),
                    })?;
                (info, false)
            }
        };

    print_node_coverage(name_or_id, node_id, &info, cached);
    Ok(())
}

/// Print a coverage summary across all non-leaf nodes.
#[cfg(feature = "embeddings")]
fn cmd_all(
    db: &SqliteGraph,
    provider: &dyn ail_search::EmbeddingProvider,
    cfg: &CoverageConfig,
    config_hash: &str,
) -> Result<(), CliError> {
    use ail_coverage::compute_coverage;

    let all_ids = db.all_node_ids().map_err(|e| CliError::Pipeline {
        errors: format!("all_node_ids: {e}"),
    })?;

    let total = all_ids.len();
    let mut counts = StatusCounts::default();

    for id in all_ids {
        let id_str = id.to_string();

        // Leaf check: if the node has no children, count as N/A and skip.
        let children = db.children(id).map_err(|e| CliError::Pipeline {
            errors: format!("children({id}): {e}"),
        })?;
        if children.is_empty() {
            counts.leaf += 1;
            continue;
        }

        // Try cache, then compute.
        let info = match db
            .load_coverage(&id_str, config_hash)
            .map_err(|e| CliError::Pipeline {
                errors: format!("load_coverage({id}): {e}"),
            })? {
            Some(cached) => cached,
            None => {
                let result =
                    compute_coverage(db, provider, id, &cfg.extra_concepts).map_err(|e| {
                        CliError::Pipeline {
                            errors: format!("compute_coverage({id}): {e}"),
                        }
                    })?;
                let info = result.into_info(cfg, config_hash.to_owned());
                db.save_coverage(&id_str, &info)
                    .map_err(|e| CliError::Pipeline {
                        errors: format!("save_coverage({id}): {e}"),
                    })?;
                info
            }
        };

        counts.tally(&info.status);
    }

    let mut summary = format!(
        "Coverage summary ({total} nodes): Full {}  Partial {}  Weak {}  N/A {}",
        counts.full, counts.partial, counts.weak, counts.leaf
    );
    if counts.unavailable > 0 {
        summary.push_str(&format!("  Unavailable {}", counts.unavailable));
    }
    println!("{summary}");
    Ok(())
}

/// Recompute coverage for every non-leaf node and persist results.
#[cfg(feature = "embeddings")]
fn cmd_warm_cache(
    db: &SqliteGraph,
    provider: &dyn ail_search::EmbeddingProvider,
    cfg: &CoverageConfig,
    config_hash: &str,
) -> Result<(), CliError> {
    use ail_coverage::compute_coverage;

    let all_ids = db.all_node_ids().map_err(|e| CliError::Pipeline {
        errors: format!("all_node_ids: {e}"),
    })?;

    // Collect non-leaf node ids.
    let mut non_leaf_ids = Vec::new();
    for id in all_ids {
        let children = db.children(id).map_err(|e| CliError::Pipeline {
            errors: format!("children({id}): {e}"),
        })?;
        if !children.is_empty() {
            non_leaf_ids.push(id);
        }
    }

    let n = non_leaf_ids.len();
    let started = Instant::now();

    for (i, id) in non_leaf_ids.iter().enumerate() {
        let id_str = id.to_string();
        // Resolve a display name.
        let display = db
            .get_node(*id)
            .ok()
            .flatten()
            .and_then(|node| node.metadata.name.clone())
            .unwrap_or_else(|| id_str.clone());

        let result = compute_coverage(db, provider, *id, &cfg.extra_concepts).map_err(|e| {
            CliError::Pipeline {
                errors: format!("compute_coverage({display}): {e}"),
            }
        })?;
        let status = CoverageStatus::from_score(result.score, cfg);
        let info = result.into_info(cfg, config_hash.to_owned());
        db.save_coverage(&id_str, &info)
            .map_err(|e| CliError::Pipeline {
                errors: format!("save_coverage({display}): {e}"),
            })?;

        println!("[{}/{}] {}: {}", i + 1, n, display, status.label());
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!("Warmed {n} entries in {elapsed:.2}s");
    Ok(())
}

// ─── Output formatting ────────────────────────────────────────────────────────

#[cfg(feature = "embeddings")]
fn print_node_coverage(name_or_id: &str, node_id: NodeId, info: &CoverageInfo, cached: bool) {
    println!("Coverage for {} (id: {})", name_or_id, node_id);

    let score_str = match info.score {
        Some(s) => format!("{:.2}", s),
        None => "N/A".to_owned(),
    };
    let status_label = info.status.label();
    let cache_note = if cached { " (cached)" } else { " (computed)" };
    println!("Score:  {status_label}  ({score_str}){cache_note}");

    if info.empty_parent {
        println!("  Note: parent intent vector is empty; score is definitionally 0.0.");
    }
    if info.degenerate_basis_fallback {
        println!("  Note: basis degenerated; averaged-cosine fallback used.");
    }

    if !info.child_contributions.is_empty() {
        println!("Children:");
        for child in &info.child_contributions {
            let preview = format_child_preview(&child.intent_preview);
            println!("  {} ... {:.2}", preview, child.projection_magnitude);
        }
    }

    if !info.missing_aspects.is_empty() {
        println!("Missing aspects:");
        for aspect in &info.missing_aspects {
            println!("  {}  ({:.2})", aspect.concept, aspect.similarity);
        }
    }
}

/// Truncate an intent preview to at most 50 Unicode codepoints.
///
/// Uses char-safe slicing so no panic occurs on multi-byte codepoints.
/// Public for unit tests; otherwise crate-private.
#[cfg(feature = "embeddings")]
pub(crate) fn format_child_preview(intent: &str) -> String {
    const MAX_CHARS: usize = 50;
    if intent.chars().count() > MAX_CHARS {
        let head: String = intent.chars().take(MAX_CHARS).collect();
        format!("{head}…")
    } else {
        intent.to_owned()
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Resolve a display name or id string to a [`NodeId`].
///
/// Tries exact UUID parse first, then falls back to name/intent matching
/// (same approach as `context.rs`).
#[cfg(feature = "embeddings")]
fn resolve_node_id(db: &SqliteGraph, name_or_id: &str) -> Result<NodeId, CliError> {
    // Try to parse as a UUID directly.
    if let Ok(id) = name_or_id.parse::<NodeId>() {
        if db
            .get_node(id)
            .map_err(|e| CliError::Pipeline {
                errors: format!("get_node: {e}"),
            })?
            .is_some()
        {
            return Ok(id);
        }
    }

    // Fall back to name/intent search across all nodes.
    let ids = db.all_node_ids().map_err(|e| CliError::Pipeline {
        errors: format!("all_node_ids: {e}"),
    })?;

    for id in ids {
        let node = match db.get_node(id).map_err(|e| CliError::Pipeline {
            errors: format!("get_node: {e}"),
        })? {
            Some(n) => n,
            None => continue,
        };

        let matches_name = node
            .metadata
            .name
            .as_deref()
            .map(|n| n.eq_ignore_ascii_case(name_or_id))
            .unwrap_or(false);
        let matches_intent = node.intent.eq_ignore_ascii_case(name_or_id);

        if matches_name || matches_intent {
            return Ok(id);
        }
    }

    Err(CliError::Pipeline {
        errors: format!("node '{name_or_id}' not found"),
    })
}

/// Count non-leaf nodes (nodes that have at least one child).
#[cfg(feature = "embeddings")]
fn count_non_leaf_nodes(db: &SqliteGraph) -> usize {
    let Ok(ids) = db.all_node_ids() else {
        return 0;
    };
    ids.into_iter()
        .filter(|&id| db.children(id).map(|c| !c.is_empty()).unwrap_or(false))
        .count()
}

/// Running counts of coverage statuses for the `--all` summary.
#[cfg(feature = "embeddings")]
#[derive(Default)]
struct StatusCounts {
    full: usize,
    partial: usize,
    weak: usize,
    leaf: usize,
    unavailable: usize,
}

#[cfg(feature = "embeddings")]
impl StatusCounts {
    fn tally(&mut self, status: &CoverageStatus) {
        match status {
            CoverageStatus::Full => self.full += 1,
            CoverageStatus::Partial => self.partial += 1,
            CoverageStatus::Weak => self.weak += 1,
            CoverageStatus::Leaf => self.leaf += 1,
            CoverageStatus::Unavailable => self.unavailable += 1,
        }
    }
}

// ─── TOML config parser ───────────────────────────────────────────────────────

/// Read the `[coverage]` section from `root/ail.config.toml`.
///
/// Returns [`CoverageConfig::default`] when the file is absent, the section is
/// missing, or any field fails to parse (per-field fallback — never panics).
pub fn read_coverage_config(root: &Path) -> CoverageConfig {
    let text = match std::fs::read_to_string(root.join("ail.config.toml")) {
        Ok(t) => t,
        Err(_) => return CoverageConfig::default(),
    };

    let mut cfg = CoverageConfig::default();
    let mut in_coverage = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Section header detection.
        if line.starts_with('[') && line.ends_with(']') {
            in_coverage = line == "[coverage]";
            continue;
        }
        if !in_coverage {
            continue;
        }

        // Parse key = value (strip inline comments).
        let (key, rest) = match line.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        let key = key.trim();
        let value_raw = rest.split('#').next().unwrap_or(rest).trim();

        match key {
            "enabled" => {
                if let Ok(b) = value_raw.parse::<bool>() {
                    cfg.enabled = b;
                }
            }
            "threshold_full" => {
                if let Ok(f) = value_raw.parse::<f32>() {
                    cfg.threshold_full = f.clamp(0.0, 1.0);
                }
            }
            "threshold_partial" => {
                if let Ok(f) = value_raw.parse::<f32>() {
                    cfg.threshold_partial = f.clamp(0.0, 1.0);
                }
            }
            "extra_concepts" => {
                cfg.extra_concepts = parse_toml_string_array(value_raw);
            }
            _ => {}
        }
    }

    cfg
}

/// Parse a TOML inline string array of the form `["a", "b", "c"]`.
///
/// Tolerant: returns an empty vec on any structural problem.
fn parse_toml_string_array(raw: &str) -> Vec<String> {
    // Expect brackets.
    let inner = raw.trim();
    let inner = inner.strip_prefix('[').and_then(|s| s.strip_suffix(']'));
    let inner = match inner {
        Some(s) => s,
        None => return vec![],
    };

    inner
        .split(',')
        .filter_map(|token| {
            let s = token.trim();
            // Strip surrounding quotes (single or double).
            let s = s
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(s);
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .collect()
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // `format_child_preview` is always compiled (feature-gated at the function
    // definition site) but the test only runs when the embeddings feature is
    // enabled.  Under non-embeddings builds the function does not exist, so the
    // test module must be guarded by the same feature flag.
    #[cfg(feature = "embeddings")]
    use super::format_child_preview;

    /// ASCII string shorter than 50 chars — returned unchanged.
    #[cfg(feature = "embeddings")]
    #[test]
    fn format_child_preview_short_ascii() {
        let s = "validate transfer amount";
        assert_eq!(format_child_preview(s), s);
    }

    /// ASCII string exactly 50 chars — returned unchanged (no truncation).
    #[cfg(feature = "embeddings")]
    #[test]
    fn format_child_preview_exactly_50_chars() {
        let s = "a".repeat(50);
        let result = format_child_preview(&s);
        assert_eq!(result, s, "exactly 50 chars must not be truncated");
    }

    /// ASCII string longer than 50 chars — truncated with ellipsis.
    #[cfg(feature = "embeddings")]
    #[test]
    fn format_child_preview_long_ascii() {
        let s = "a".repeat(60);
        let result = format_child_preview(&s);
        assert!(
            result.ends_with('…'),
            "long string must end with ellipsis, got: {result:?}"
        );
        // The head before the ellipsis is exactly 50 'a' chars.
        let head: &str = result.trim_end_matches('…');
        assert_eq!(head.len(), 50);
    }

    /// Multi-byte string: 20 Japanese characters, each 3 bytes (60 bytes total
    /// but only 20 Unicode codepoints) — must NOT be truncated (20 < 50 chars).
    ///
    /// This is the regression test for C1: prior byte-slice code would have
    /// truncated at byte 50 (mid-codepoint on a 3-byte char), causing a panic.
    #[cfg(feature = "embeddings")]
    #[test]
    fn format_child_preview_multibyte_short_in_chars() {
        // Each '日' is 3 bytes but 1 char; 20 of them = 60 bytes, 20 chars.
        let s = "日".repeat(20);
        assert_eq!(s.len(), 60, "precondition: 60 bytes");
        assert_eq!(s.chars().count(), 20, "precondition: 20 chars");

        // Must not panic, and must NOT truncate (20 < 50 chars).
        let result = format_child_preview(&s);
        assert_eq!(result, s, "20-char multi-byte string must not be truncated");
    }

    /// Multi-byte string: 55 Japanese characters (165 bytes) — must be
    /// truncated to 50 chars + ellipsis without panicking.
    #[cfg(feature = "embeddings")]
    #[test]
    fn format_child_preview_multibyte_long_in_chars() {
        let s = "日".repeat(55);
        assert_eq!(s.len(), 165, "precondition: 165 bytes");

        // Must not panic.
        let result = format_child_preview(&s);
        assert!(result.ends_with('…'), "must end with ellipsis");

        // Head must be exactly 50 '日' chars (150 bytes).
        let head: &str = result.trim_end_matches('…');
        assert_eq!(head.chars().count(), 50);
        assert_eq!(head.len(), 150, "50 × 3-byte chars = 150 bytes");
    }

    /// Verify `parse_toml_string_array` handles typical inline arrays.
    #[test]
    fn parse_toml_string_array_basic() {
        use super::parse_toml_string_array;
        let result = parse_toml_string_array(r#"["auth", "payments", "retry"]"#);
        assert_eq!(result, vec!["auth", "payments", "retry"]);
    }
}
