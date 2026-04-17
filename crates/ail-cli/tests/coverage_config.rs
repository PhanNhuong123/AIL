//! Tests for `read_coverage_config` — the TOML `[coverage]` section parser.
//!
//! These tests are feature-independent (no embeddings required) because they
//! exercise pure TOML-parsing logic against the file system.

use std::fs;
use std::path::Path;

use ail_cli::read_coverage_config;
use ail_graph::cic::CoverageConfig;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn write_config(root: &Path, body: &str) {
    fs::write(root.join("ail.config.toml"), body).unwrap();
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn t132_read_coverage_config_parses_extra_concepts() {
    let tmp = tempfile::tempdir().unwrap();

    write_config(
        tmp.path(),
        "[coverage]\nthreshold_full = 0.95\nextra_concepts = [\"a\", \"b\"]\n",
    );

    let cfg = read_coverage_config(tmp.path());

    assert!(
        (cfg.threshold_full - 0.95).abs() < 1e-5,
        "threshold_full should be 0.95, got {}",
        cfg.threshold_full
    );
    assert_eq!(
        cfg.extra_concepts,
        vec!["a".to_owned(), "b".to_owned()],
        "extra_concepts mismatch"
    );
    // Defaults preserved for unspecified fields.
    assert!(cfg.enabled, "enabled should default to true");
    assert!(
        (cfg.threshold_partial - CoverageConfig::default().threshold_partial).abs() < 1e-5,
        "threshold_partial should be default"
    );
}

#[test]
fn t132_read_coverage_config_defaults_on_missing_section() {
    let tmp = tempfile::tempdir().unwrap();

    // Write a config that has a different section but NOT [coverage].
    write_config(tmp.path(), "[database]\nbackend = \"auto\"\n");

    let cfg = read_coverage_config(tmp.path());
    let expected = CoverageConfig::default();

    assert_eq!(cfg.enabled, expected.enabled);
    assert!(
        (cfg.threshold_full - expected.threshold_full).abs() < 1e-5,
        "threshold_full should be default"
    );
    assert!(
        (cfg.threshold_partial - expected.threshold_partial).abs() < 1e-5,
        "threshold_partial should be default"
    );
    assert!(
        cfg.extra_concepts.is_empty(),
        "extra_concepts should be empty"
    );
}
