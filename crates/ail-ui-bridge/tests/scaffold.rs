//! Tests for `scaffold_project` (closes review finding **N2**).
//!
//! Pure helpers (`validate_name`, `validate_kind`, `render_skeleton`,
//! `render_config`) plus the `scaffold_project` command end-to-end with a
//! tempdir parent.

#![cfg(feature = "tauri-commands")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ail_ui_bridge::scaffold::{
    render_config, render_skeleton, scaffold_project, validate_kind, validate_name,
};
use ail_ui_bridge::types::scaffold::{ProjectScaffoldRequest, ProjectScaffoldResult};

// ---------------------------------------------------------------------------
// Tempdir helper — process-unique, no external dep
// ---------------------------------------------------------------------------

fn unique_tempdir(label: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = env::temp_dir().join(format!("ail-scaffold-test-{label}-{}-{:x}", seq, now_ns));
    fs::create_dir_all(&dir).expect("tempdir create");
    dir
}

// ---------------------------------------------------------------------------
// validate_name
// ---------------------------------------------------------------------------

#[test]
fn validate_name_accepts_basic_identifier() {
    assert!(validate_name("wallet_service").is_ok());
    assert!(validate_name("Wallet").is_ok());
    assert!(validate_name("a").is_ok());
}

#[test]
fn validate_name_rejects_empty() {
    assert!(validate_name("").is_err());
}

#[test]
fn validate_name_rejects_leading_digit() {
    assert!(validate_name("1wallet").is_err());
}

#[test]
fn validate_name_rejects_special_chars() {
    assert!(validate_name("wallet-service").is_err());
    assert!(validate_name("wallet.service").is_err());
    assert!(validate_name("wallet/service").is_err());
    assert!(validate_name("wallet service").is_err());
}

// Closes acceptance review MINOR-1 (2026-05-01): explicit path-traversal
// guard. The behavior already passes through `validate_name` (which rejects
// `.` and `/`), but a named test pins the security contract so a future
// relaxation of the character set cannot accidentally re-open path traversal
// via `<parent>/..` or `<parent>/../foo` writes.
#[test]
fn validate_name_rejects_path_traversal() {
    assert!(validate_name("..").is_err(), "bare `..` must be rejected");
    assert!(validate_name(".").is_err(), "bare `.` must be rejected");
    assert!(
        validate_name("../foo").is_err(),
        "`../foo` must be rejected"
    );
    assert!(
        validate_name("..\\foo").is_err(),
        "`..\\foo` must be rejected (Windows separator)"
    );
    assert!(
        validate_name("foo/..").is_err(),
        "`foo/..` must be rejected"
    );
    assert!(
        validate_name("foo\\bar").is_err(),
        "Windows backslash must be rejected"
    );
}

#[test]
fn validate_name_rejects_overlong() {
    let too_long: String = "a".repeat(65);
    assert!(validate_name(&too_long).is_err());
    let max_ok: String = "a".repeat(64);
    assert!(validate_name(&max_ok).is_ok());
}

// ---------------------------------------------------------------------------
// validate_kind
// ---------------------------------------------------------------------------

#[test]
fn validate_kind_accepts_known_kinds() {
    assert_eq!(validate_kind("module").unwrap(), "module");
    assert_eq!(validate_kind("function").unwrap(), "function");
    assert_eq!(validate_kind("rule").unwrap(), "rule");
    assert_eq!(validate_kind("test").unwrap(), "test");
}

#[test]
fn validate_kind_rejects_unknown() {
    assert!(validate_kind("class").is_err());
    assert!(validate_kind("MODULE").is_err());
    assert!(validate_kind("").is_err());
}

// ---------------------------------------------------------------------------
// render_skeleton — deterministic output shape
// ---------------------------------------------------------------------------

#[test]
fn render_skeleton_module_no_description() {
    let out = render_skeleton("module", "wallet", "");
    assert!(out.starts_with("module wallet {"));
    assert!(out.contains("Define functions"));
}

#[test]
fn render_skeleton_module_with_description() {
    let out = render_skeleton("module", "wallet", "Wallet service skeleton.");
    assert!(out.starts_with("// Wallet service skeleton.\n\n"));
    assert!(out.contains("module wallet {"));
}

/// Acceptance pass 2026-05-02 (story A): a description containing newlines
/// must produce a comment block where every emitted line carries `// ` so
/// the parser does not see a bare second line. Previously the single
/// `out.push_str("// ")` only covered the first line and the second was
/// emitted as a non-comment, breaking parses of any project scaffolded
/// with a multi-line description in QuickCreate.
#[test]
fn render_skeleton_multiline_description_keeps_comment_marker_on_each_line() {
    let out = render_skeleton(
        "module",
        "wallet",
        "Wallet service skeleton.\nSecond paragraph with details.",
    );
    assert!(
        out.starts_with("// Wallet service skeleton.\n// Second paragraph with details.\n\n"),
        "expected each line to start with `// `, got: {out:?}"
    );
    // No bare line directly above the `module wallet {` opener.
    assert!(out.contains("\n\nmodule wallet {"));
}

#[test]
fn render_skeleton_three_line_description_keeps_comment_marker_on_each_line() {
    let out = render_skeleton("module", "wallet", "line one\nline two\nline three");
    let expected_prefix = "// line one\n// line two\n// line three\n\n";
    assert!(
        out.starts_with(expected_prefix),
        "expected `{expected_prefix}`, got: {out:?}"
    );
}

#[test]
fn render_skeleton_function_emits_function_block() {
    let out = render_skeleton("function", "deposit", "");
    assert!(out.contains("function deposit()"));
}

#[test]
fn render_skeleton_rule_emits_rule_block() {
    let out = render_skeleton("rule", "non_negative", "");
    assert!(out.contains("rule non_negative {"));
    assert!(out.contains("postcondition"));
}

#[test]
fn render_skeleton_test_emits_test_block() {
    let out = render_skeleton("test", "transfer_ok", "");
    assert!(out.contains("test transfer_ok {"));
}

#[test]
fn render_skeleton_trims_description_whitespace() {
    let out = render_skeleton("module", "x", "   leading and trailing   ");
    assert!(out.starts_with("// leading and trailing\n\n"));
}

// ---------------------------------------------------------------------------
// render_config
// ---------------------------------------------------------------------------

#[test]
fn render_config_includes_name_and_version() {
    let out = render_config("wallet");
    assert!(out.contains("[project]"));
    assert!(out.contains("name = \"wallet\""));
    assert!(out.contains("version = \"0.1.0\""));
}

// ---------------------------------------------------------------------------
// scaffold_project — happy path
// ---------------------------------------------------------------------------

#[test]
fn scaffold_project_writes_skeleton_and_config() {
    let parent = unique_tempdir("happy");
    let request = ProjectScaffoldRequest {
        parent_dir: parent.to_string_lossy().into_owned(),
        kind: "module".to_string(),
        name: "wallet".to_string(),
        description: "Demo wallet".to_string(),
    };

    let result: ProjectScaffoldResult = scaffold_project(request).expect("scaffold ok");

    assert_eq!(
        PathBuf::from(&result.project_dir),
        parent.join("wallet"),
        "project_dir should equal <parent>/wallet"
    );
    assert_eq!(
        PathBuf::from(&result.ail_file),
        parent.join("wallet").join("src").join("wallet.ail"),
        "ail_file should be src/wallet.ail under project dir"
    );

    let ail_body = fs::read_to_string(&result.ail_file).expect("ail file readable");
    assert!(ail_body.starts_with("// Demo wallet\n\n"));
    assert!(ail_body.contains("module wallet {"));

    let config_body =
        fs::read_to_string(parent.join("wallet").join("ail.config.toml")).expect("config readable");
    assert!(config_body.contains("name = \"wallet\""));

    let _ = fs::remove_dir_all(&parent);
}

// ---------------------------------------------------------------------------
// scaffold_project — error paths
// ---------------------------------------------------------------------------

#[test]
fn scaffold_project_rejects_invalid_name() {
    let parent = unique_tempdir("bad-name");
    let request = ProjectScaffoldRequest {
        parent_dir: parent.to_string_lossy().into_owned(),
        kind: "module".to_string(),
        name: "1bad".to_string(),
        description: "".to_string(),
    };
    assert!(scaffold_project(request).is_err());
    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn scaffold_project_rejects_invalid_kind() {
    let parent = unique_tempdir("bad-kind");
    let request = ProjectScaffoldRequest {
        parent_dir: parent.to_string_lossy().into_owned(),
        kind: "class".to_string(),
        name: "wallet".to_string(),
        description: "".to_string(),
    };
    assert!(scaffold_project(request).is_err());
    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn scaffold_project_rejects_missing_parent() {
    let parent = unique_tempdir("missing-parent");
    let nonexistent = parent.join("does-not-exist");
    let request = ProjectScaffoldRequest {
        parent_dir: nonexistent.to_string_lossy().into_owned(),
        kind: "module".to_string(),
        name: "wallet".to_string(),
        description: "".to_string(),
    };
    assert!(scaffold_project(request).is_err());
    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn scaffold_project_rejects_existing_target() {
    let parent = unique_tempdir("collision");
    fs::create_dir_all(parent.join("wallet")).expect("pre-create");
    let request = ProjectScaffoldRequest {
        parent_dir: parent.to_string_lossy().into_owned(),
        kind: "module".to_string(),
        name: "wallet".to_string(),
        description: "".to_string(),
    };
    let err = scaffold_project(request).expect_err("should refuse to overwrite");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("already exists"),
        "error should mention existing target: {msg}"
    );
    let _ = fs::remove_dir_all(&parent);
}

// ---------------------------------------------------------------------------
// DTO serde — camelCase wire format
// ---------------------------------------------------------------------------

#[test]
fn project_scaffold_request_serializes_camelcase() {
    let req = ProjectScaffoldRequest {
        parent_dir: "/tmp/x".to_string(),
        kind: "module".to_string(),
        name: "wallet".to_string(),
        description: "demo".to_string(),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"parentDir\""));
    assert!(json.contains("\"kind\""));
    assert!(json.contains("\"name\""));
    assert!(json.contains("\"description\""));
    assert!(!json.contains("\"parent_dir\""));
}

#[test]
fn project_scaffold_result_serializes_camelcase() {
    let res = ProjectScaffoldResult {
        project_dir: "/tmp/x/wallet".to_string(),
        ail_file: "/tmp/x/wallet/src/wallet.ail".to_string(),
    };
    let json = serde_json::to_string(&res).unwrap();
    assert!(json.contains("\"projectDir\""));
    assert!(json.contains("\"ailFile\""));
}
