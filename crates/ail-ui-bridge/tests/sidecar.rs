//! Unit tests for Phase 16.5 sidecar helpers and DTOs (SC1–SC12).
//!
//! Covers pure helpers only — `health_check_core` / `health_check_agent`
//! require a live `tauri::AppHandle` which cannot be constructed outside a
//! Tauri runtime. Pure extract (`seed_sidecar_nonce`, `next_sidecar_run_id_string`,
//! `parse_ail_dev_mode`, `parse_version_line`) and DTO serde are exercised here.
//!
//! Env-var tests (SC4, SC5) use a per-test mutex to avoid races from parallel
//! test execution modifying `AIL_DEV`.

#![cfg(feature = "tauri-commands")]

use std::sync::{Mutex, OnceLock};

use ail_ui_bridge::sidecar::{
    next_sidecar_run_id_string, parse_ail_dev_mode, parse_version_line, seed_sidecar_nonce,
};
use ail_ui_bridge::types::sidecar_result::{HealthCheckPayload, SidecarMode};
use ail_ui_bridge::BridgeStateInner;

/// Per-test mutex serializing writes to the `AIL_DEV` environment variable so
/// SC4 and SC5 cannot race each other when run in parallel.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

// ---------------------------------------------------------------------------
// SC1 — seed_sidecar_nonce is non-zero
// ---------------------------------------------------------------------------

#[test]
fn seed_sidecar_nonce_nonzero() {
    let n = seed_sidecar_nonce();
    assert_ne!(n, 0, "nonce should be non-zero on a normal system");
}

// ---------------------------------------------------------------------------
// SC2 — next_sidecar_run_id_string has prefix and increments
// ---------------------------------------------------------------------------

#[test]
fn next_sidecar_run_id_has_prefix_and_increments() {
    let nonce: u64 = 0xABCD_1234_5678_EF00;
    let mut seq: u64 = 0;

    let first = next_sidecar_run_id_string(&mut seq, nonce);
    let second = next_sidecar_run_id_string(&mut seq, nonce);

    assert!(
        first.starts_with("sidecar-"),
        "first run id must have 'sidecar-' prefix: {first}"
    );
    assert!(
        second.starts_with("sidecar-"),
        "second run id must have 'sidecar-' prefix: {second}"
    );
    assert_ne!(first, second, "successive run ids must differ");
    assert_eq!(seq, 2, "seq must have incremented to 2");

    // First call: seq=1, so prefix is "sidecar-1-{nonce:x}"
    let expected_first = format!("sidecar-1-{:x}", nonce);
    assert_eq!(first, expected_first);
}

// ---------------------------------------------------------------------------
// SC3 — format is "sidecar-<hex>-<hex>" with three dash-separated parts
// ---------------------------------------------------------------------------

#[test]
fn next_sidecar_run_id_format_is_hex_separated() {
    let nonce: u64 = 0xDEAD_BEEF_CAFE_1234;
    let mut seq: u64 = 0;
    let id = next_sidecar_run_id_string(&mut seq, nonce);

    // "sidecar-<seq_hex>-<nonce_hex>" → split on '-' yields 3 parts
    let parts: Vec<&str> = id.splitn(3, '-').collect();
    assert_eq!(
        parts.len(),
        3,
        "run id must have 3 '-'-separated parts: {id}"
    );
    assert_eq!(parts[0], "sidecar");
    // seq part and nonce part must be valid hex
    u64::from_str_radix(parts[1], 16).expect("seq portion must be valid hex");
    u64::from_str_radix(parts[2], 16).expect("nonce portion must be valid hex");
}

// ---------------------------------------------------------------------------
// SC4 — parse_ail_dev_mode returns false when AIL_DEV is unset
// ---------------------------------------------------------------------------

#[test]
fn parse_ail_dev_mode_returns_false_when_unset() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
    std::env::remove_var("AIL_DEV");
    assert!(!parse_ail_dev_mode(), "must be false when AIL_DEV is unset");
}

// ---------------------------------------------------------------------------
// SC5 — parse_ail_dev_mode returns true only for "1"
// ---------------------------------------------------------------------------

#[test]
fn parse_ail_dev_mode_returns_true_only_when_one() {
    let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());

    std::env::set_var("AIL_DEV", "1");
    assert!(parse_ail_dev_mode(), "must be true for \"1\"");

    for val in ["", "0", "true", "yes", "YES", "True", "on"] {
        std::env::set_var("AIL_DEV", val);
        assert!(!parse_ail_dev_mode(), "must be false for AIL_DEV={val:?}");
    }

    std::env::remove_var("AIL_DEV");
}

// ---------------------------------------------------------------------------
// SC6 — parse_version_line extracts version for valid input
// ---------------------------------------------------------------------------

#[test]
fn parse_version_line_extracts_version() {
    assert_eq!(parse_version_line("ail 0.1.0\n"), Some("0.1.0".to_string()));
    assert_eq!(
        parse_version_line("ail_agent 0.1.0"),
        Some("0.1.0".to_string())
    );
    // One token — no version
    assert_eq!(parse_version_line("foo"), None);
    // Three tokens — rejected
    assert_eq!(parse_version_line("a b c"), None);
}

// ---------------------------------------------------------------------------
// SC7 — parse_version_line handles empty input
// ---------------------------------------------------------------------------

#[test]
fn parse_version_line_handles_empty_input() {
    assert_eq!(parse_version_line(""), None);
    assert_eq!(parse_version_line("   "), None);
    assert_eq!(parse_version_line("\n\n"), None);
}

// ---------------------------------------------------------------------------
// SC8 — HealthCheckPayload serializes with camelCase fields
// ---------------------------------------------------------------------------

#[test]
fn health_check_payload_serde_camel_case() {
    let p = HealthCheckPayload {
        component: "ail-core".to_string(),
        ok: true,
        mode: SidecarMode::Bundled,
        version: Some("0.1.0".to_string()),
        error: None,
    };
    let v: serde_json::Value = serde_json::to_value(&p).unwrap();

    assert_eq!(v["component"], "ail-core");
    assert_eq!(v["ok"], true);
    assert_eq!(v["mode"], "bundled");
    assert_eq!(v["version"], "0.1.0");
    // error: None → skip_serializing_if omits the key
    assert!(
        v.get("error").is_none(),
        "error must be omitted when None; got {v}"
    );
}

// ---------------------------------------------------------------------------
// SC9 — SidecarMode serializes as lowercase camelCase
// ---------------------------------------------------------------------------

#[test]
fn sidecar_mode_serializes_lowercase() {
    let bundled = serde_json::to_string(&SidecarMode::Bundled).unwrap();
    let dev = serde_json::to_string(&SidecarMode::Dev).unwrap();
    assert_eq!(bundled, r#""bundled""#);
    assert_eq!(dev, r#""dev""#);
}

// ---------------------------------------------------------------------------
// SC10 — HealthCheckPayload with error omits version key
// ---------------------------------------------------------------------------

#[test]
fn health_check_payload_with_error_omits_version() {
    let p = HealthCheckPayload {
        component: "ail-core".to_string(),
        ok: false,
        mode: SidecarMode::Dev,
        version: None,
        error: Some("spawn failed: No such file or directory".to_string()),
    };
    let v: serde_json::Value = serde_json::to_value(&p).unwrap();
    assert_eq!(v["ok"], false);
    assert!(
        v.get("version").is_none(),
        "version must be omitted when None; got {v}"
    );
    assert_eq!(v["error"], "spawn failed: No such file or directory");
}

// ---------------------------------------------------------------------------
// SC11 — HealthCheckPayload round-trip
// ---------------------------------------------------------------------------

#[test]
fn health_check_payload_roundtrip() {
    let original = HealthCheckPayload {
        component: "ail-agent".to_string(),
        ok: true,
        mode: SidecarMode::Dev,
        version: Some("0.1.0".to_string()),
        error: None,
    };
    let json = serde_json::to_value(&original).unwrap();
    let back: HealthCheckPayload = serde_json::from_value(json).unwrap();
    assert_eq!(original, back);
}

// ---------------------------------------------------------------------------
// SC12 — BridgeStateInner struct literal compiles with sidecar fields
// ---------------------------------------------------------------------------

#[test]
fn bridge_state_inner_has_sidecar_fields() {
    let inner = BridgeStateInner {
        project_path: None,
        graph_json: None,
        watcher: None,
        load_generation: 0,
        agent_run: None,
        agent_run_seq: 0,
        agent_id_nonce: 0,
        verifier_run: None,
        verifier_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        verifier_run_seq: 0,
        verifier_id_nonce: 0,
        sheaf_run: None,
        sheaf_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        sheaf_run_seq: 0,
        sheaf_id_nonce: 0,
        sidecar_health_seq: 0,
        sidecar_id_nonce: 42,
    };

    assert_eq!(
        inner.sidecar_health_seq, 0,
        "sidecar_health_seq must start at 0"
    );
    assert_eq!(
        inner.sidecar_id_nonce, 42,
        "sidecar_id_nonce must match what was set"
    );
}
