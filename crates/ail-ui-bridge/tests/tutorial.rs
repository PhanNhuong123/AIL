//! Tests for `get_tutorial_path` pure helpers (closes review finding
//! **N1.b** "Try the tutorial").
//!
//! The Tauri command itself requires a live `AppHandle` and is therefore
//! exercised from the IDE side. Pure helpers (`find_tutorial_in_workspace`,
//! `resolve_tutorial_path_dev`) are covered here.

#![cfg(feature = "tauri-commands")]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ail_ui_bridge::tutorial::{find_tutorial_in_workspace, resolve_tutorial_path_dev};

fn unique_tempdir(label: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = env::temp_dir().join(format!(
        "ail-tutorial-test-{label}-{}-{:x}",
        seq, now_ns
    ));
    fs::create_dir_all(&dir).expect("tempdir create");
    dir
}

// ---------------------------------------------------------------------------
// find_tutorial_in_workspace
// ---------------------------------------------------------------------------

#[test]
fn find_tutorial_returns_match_when_present() {
    // Build: tmp/.../foo/bar/, with examples/wallet_service nested under tmp/...
    let root = unique_tempdir("present");
    let nested = root.join("foo").join("bar");
    fs::create_dir_all(&nested).unwrap();
    let tutorial = root.join("examples").join("wallet_service");
    fs::create_dir_all(&tutorial).unwrap();

    let found = find_tutorial_in_workspace(&nested).expect("walk-up should find tutorial");
    let canonical_found = canonicalize_or_self(&found);
    let canonical_expected = canonicalize_or_self(&tutorial);
    assert_eq!(canonical_found, canonical_expected);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn find_tutorial_returns_none_when_absent() {
    let root = unique_tempdir("absent");
    let nested = root.join("foo");
    fs::create_dir_all(&nested).unwrap();

    let found = find_tutorial_in_workspace(&nested);
    assert!(found.is_none(), "should not invent a tutorial path");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn find_tutorial_prefers_closest_match() {
    // Two candidates: one at tmp/closest/examples/wallet_service AND one
    // at tmp/closest/inner/examples/wallet_service. Walking up from inner
    // returns the inner one first.
    let root = unique_tempdir("closest");
    let inner = root.join("inner");
    fs::create_dir_all(&inner).unwrap();
    let outer_tutorial = root.join("examples").join("wallet_service");
    fs::create_dir_all(&outer_tutorial).unwrap();
    let inner_tutorial = inner.join("examples").join("wallet_service");
    fs::create_dir_all(&inner_tutorial).unwrap();

    let found = find_tutorial_in_workspace(&inner).expect("found inner");
    assert_eq!(canonicalize_or_self(&found), canonicalize_or_self(&inner_tutorial));

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// resolve_tutorial_path_dev — covers CARGO_MANIFEST_DIR fallback
// ---------------------------------------------------------------------------

#[test]
fn resolve_tutorial_path_dev_finds_workspace_example() {
    // CARGO_MANIFEST_DIR is set by cargo when running tests, so this hits
    // the workspace's real `examples/wallet_service`.
    let resolved = resolve_tutorial_path_dev();
    assert!(
        resolved.is_some(),
        "workspace tutorial should be resolvable from cargo test context"
    );
    let path = resolved.unwrap();
    assert!(path.is_dir(), "resolved path must be a directory: {path:?}");
    assert!(
        path.ends_with("examples/wallet_service")
            || path.ends_with("examples\\wallet_service"),
        "resolved path must end with examples/wallet_service: {path:?}"
    );
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn canonicalize_or_self(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}
