//! Integration tests for the agent subprocess manager (Phase 16 task 16.1).
//!
//! Covers pure helpers only — the full `run_agent` / `cancel_agent_run`
//! commands need a `tauri::AppHandle` which cannot be instantiated outside
//! a Tauri runtime. Following the `tests/watcher.rs` precedent, the pure
//! extract (`parse_event_line`, `next_run_id_string`, `build_request_env`)
//! is exercised here; the end-to-end cancel path is covered by frontend
//! route tests in `ide/src/routes/page.test.ts`.

#![cfg(feature = "tauri-commands")]

use std::sync::atomic::Ordering;
use std::sync::Arc;

use ail_ui_bridge::agent::{
    build_request_env, next_run_id_string, parse_event_line, seed_nonce, AgentEvent,
};
use ail_ui_bridge::types::agent::{AgentCancelResult, AgentMode, AgentRunRequest};
use ail_ui_bridge::types::lens_stats::Lens;
use ail_ui_bridge::BridgeStateInner;

// ---------------------------------------------------------------------------
// parse_event_line
// ---------------------------------------------------------------------------

#[test]
fn test_parse_event_line_parses_step_message_complete() {
    let step = parse_event_line(
        r#"{"type":"step","runId":"r-1","index":1,"phase":"plan","text":"Planning..."}"#,
    )
    .expect("valid step");
    match step {
        AgentEvent::Step(p) => {
            assert_eq!(p.run_id, "r-1");
            assert_eq!(p.index, 1);
            assert_eq!(p.phase, "plan");
            assert_eq!(p.text, "Planning...");
        }
        other => panic!("expected Step, got {other:?}"),
    }

    let message = parse_event_line(
        r#"{"type":"message","runId":"r-1","messageId":"m-1","text":"Here is the plan"}"#,
    )
    .expect("valid message");
    match message {
        AgentEvent::Message(p) => {
            assert_eq!(p.message_id, "m-1");
            assert!(p.preview.is_none());
        }
        other => panic!("expected Message, got {other:?}"),
    }

    let complete =
        parse_event_line(r#"{"type":"complete","runId":"r-1","status":"done"}"#).expect("valid");
    match complete {
        AgentEvent::Complete(p) => {
            assert_eq!(p.status, "done");
            assert!(p.error.is_none());
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

#[test]
fn test_parse_event_line_returns_none_on_non_json() {
    // Python traceback / warnings / empty lines must NOT panic or corrupt
    // the reader loop — they are silently skipped (covers 3a-1 MED).
    assert!(parse_event_line("").is_none());
    assert!(parse_event_line("   ").is_none());
    assert!(parse_event_line("Planning...").is_none());
    assert!(parse_event_line("Traceback (most recent call last):").is_none());
    assert!(parse_event_line(r#"{"type":"unknown","runId":"x"}"#).is_none());
    assert!(parse_event_line(r#"{invalid json"#).is_none());
}

// ---------------------------------------------------------------------------
// next_run_id_string + seed_nonce
// ---------------------------------------------------------------------------

fn fresh_inner(nonce: u64) -> BridgeStateInner {
    BridgeStateInner {
        project_path: None,
        graph_json: None,
        watcher: None,
        load_generation: 0,
        agent_run: None,
        agent_run_seq: 0,
        agent_id_nonce: nonce,
        verifier_run: None,
        verifier_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        verifier_run_seq: 0,
        verifier_id_nonce: 0,
        sheaf_run: None,
        sheaf_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        sheaf_run_seq: 0,
        sheaf_id_nonce: 0,
        sidecar_health_seq: 0,
        sidecar_id_nonce: 0,
    }
}

#[test]
fn test_fresh_inner_has_verifier_fields() {
    let inner = fresh_inner(0xDEAD_BEEF_CAFE_1234);
    assert!(inner.verifier_run.is_none());
    assert_eq!(inner.verifier_run_seq, 0);
    assert!(
        !inner
            .verifier_cancelled
            .load(std::sync::atomic::Ordering::SeqCst),
        "verifier_cancelled fence must start false"
    );
    // verifier_id_nonce is non-deterministic in production; here we passed 0
    // explicitly to keep the test deterministic.
}

#[test]
fn test_next_run_id_is_monotonic_and_nonce_xored() {
    let nonce: u64 = 0x1234_5678_9ABC_DEF0;
    let mut inner = fresh_inner(nonce);

    let a = next_run_id_string(&mut inner);
    let b = next_run_id_string(&mut inner);
    let c = next_run_id_string(&mut inner);

    assert_ne!(a, b);
    assert_ne!(b, c);

    // a = 1 ^ nonce, b = 2 ^ nonce, c = 3 ^ nonce
    assert_eq!(a.parse::<u64>().unwrap() ^ nonce, 1);
    assert_eq!(b.parse::<u64>().unwrap() ^ nonce, 2);
    assert_eq!(c.parse::<u64>().unwrap() ^ nonce, 3);
}

#[test]
fn test_run_id_string_survives_round_trip_above_2_53() {
    // Defense against red-team B-1: a u64 near or above 2^53 must serialize
    // and parse back identically through a String round trip. Simulate a
    // realistic nonce + counter combination.
    let nonce: u64 = 0x1234_5678_9ABC_DEF0; // > 2^53
    let mut inner = fresh_inner(nonce);
    let s = next_run_id_string(&mut inner);

    let parsed: u64 = s.parse().expect("run_id must be a valid u64 decimal");
    assert_eq!(parsed, 1u64 ^ nonce);
    // Value exceeds 2^53 so JS `number` would have lost precision here.
    assert!(parsed > (1u64 << 53));
}

#[test]
fn test_seed_nonce_is_nonzero_and_varies() {
    // Not a strict uniqueness assertion (probabilistic), but seed_nonce
    // should not silently return 0 on a normal system.
    let n = seed_nonce();
    assert_ne!(n, 0, "nonce should be non-zero on a normal system");
}

// ---------------------------------------------------------------------------
// build_request_env — 16.1-A context injection + security
// ---------------------------------------------------------------------------

#[test]
fn test_build_request_env_injects_all_fields_and_no_api_keys() {
    let req = AgentRunRequest {
        text: "add rate limiter".to_string(),
        selection_kind: "function".to_string(),
        selection_id: Some("mod/fn".to_string()),
        path: vec!["mod".to_string(), "fn".to_string()],
        lens: Lens::Verify,
        mode: AgentMode::Edit,
        model: Some("anthropic:claude-sonnet-4-6".to_string()),
    };
    let env = build_request_env(&req);

    let get = |k: &str| env.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());

    assert_eq!(get("AIL_AGENT_SELECTION_KIND").as_deref(), Some("function"));
    assert_eq!(get("AIL_AGENT_SELECTION_ID").as_deref(), Some("mod/fn"));
    assert_eq!(get("AIL_AGENT_PATH").as_deref(), Some("mod|fn"));
    assert_eq!(get("AIL_AGENT_LENS").as_deref(), Some("verify"));
    assert_eq!(get("AIL_AGENT_MODE").as_deref(), Some("edit"));

    // Security invariant: NO *_API_KEY forwarding.
    for (k, _) in &env {
        assert!(
            !k.to_ascii_uppercase().contains("API_KEY"),
            "env must not forward API keys, found: {k}"
        );
        assert!(k.starts_with("AIL_AGENT_"), "only AIL_AGENT_* keys: {k}");
    }
}

#[test]
fn test_build_request_env_handles_empty_optionals() {
    let req = AgentRunRequest {
        text: "hello".to_string(),
        selection_kind: "none".to_string(),
        selection_id: None,
        path: Vec::new(),
        lens: Lens::Structure,
        mode: AgentMode::Ask,
        model: None,
    };
    let env = build_request_env(&req);
    let get = |k: &str| env.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone());
    assert_eq!(get("AIL_AGENT_SELECTION_ID").as_deref(), Some(""));
    assert_eq!(get("AIL_AGENT_PATH").as_deref(), Some(""));
    assert_eq!(get("AIL_AGENT_LENS").as_deref(), Some("structure"));
    assert_eq!(get("AIL_AGENT_MODE").as_deref(), Some("ask"));
}

// ---------------------------------------------------------------------------
// AgentCancelResult serde round trip
// ---------------------------------------------------------------------------

#[test]
fn test_agent_cancel_result_camelcase_round_trip() {
    let yes = AgentCancelResult { cancelled: true };
    let json = serde_json::to_string(&yes).unwrap();
    assert_eq!(json, r#"{"cancelled":true}"#);
    let back: AgentCancelResult = serde_json::from_str(&json).unwrap();
    assert!(back.cancelled);
}

// ---------------------------------------------------------------------------
// Cancelled-flag atomic fence semantics (B-3 resolution)
//
// The full reader loop is async and needs a live child stdout, which is hard
// to fixture portably. This test instead verifies the contract the reader
// loop relies on: once `cancelled.store(true)` happens, a subsequent
// `cancelled.load()` from any thread observes it under SeqCst ordering.
// ---------------------------------------------------------------------------

#[test]
fn test_cancelled_atomic_flag_seqcst_visibility() {
    use std::sync::atomic::AtomicBool;

    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = flag.clone();

    // Simulate cancel fence: set then await visibility in a fresh thread.
    flag.store(true, Ordering::SeqCst);
    let observed = std::thread::spawn(move || flag_clone.load(Ordering::SeqCst))
        .join()
        .expect("thread");
    assert!(observed, "SeqCst store must be visible to other threads");
}

// ---------------------------------------------------------------------------
// AgentRunRequest serde — 16.1-A canonical wire shape
// ---------------------------------------------------------------------------

#[test]
fn test_parse_event_line_extracts_run_id_for_cross_check() {
    // HIGH #2 mitigation: the reader loop cross-checks the runId carried in
    // each envelope against the run_id Rust generated. Verify each variant
    // exposes its run_id correctly (the field name is `runId` on the wire).
    let step =
        parse_event_line(r#"{"type":"step","runId":"WIRE-A","index":1,"phase":"plan","text":"x"}"#)
            .unwrap();
    let msg = parse_event_line(r#"{"type":"message","runId":"WIRE-B","messageId":"m","text":"x"}"#)
        .unwrap();
    let cmp = parse_event_line(r#"{"type":"complete","runId":"WIRE-C","status":"done"}"#).unwrap();
    match step {
        AgentEvent::Step(p) => assert_eq!(p.run_id, "WIRE-A"),
        _ => panic!("step variant"),
    }
    match msg {
        AgentEvent::Message(p) => assert_eq!(p.run_id, "WIRE-B"),
        _ => panic!("message variant"),
    }
    match cmp {
        AgentEvent::Complete(p) => assert_eq!(p.run_id, "WIRE-C"),
        _ => panic!("complete variant"),
    }
}

#[test]
fn test_agent_complete_payload_camelcase_round_trip() {
    // HIGH #3 mitigation requires emitting a synthetic
    // `agent-complete{status:"error"}` envelope. Lock the wire shape so a
    // future serde change can't silently break the frontend listener.
    use ail_ui_bridge::types::agent::AgentCompletePayload;
    let p = AgentCompletePayload {
        run_id: "r-7".to_string(),
        status: "error".to_string(),
        error: Some("agent exited without emitting complete".to_string()),
    };
    let v: serde_json::Value = serde_json::to_value(&p).unwrap();
    assert_eq!(v["runId"], "r-7");
    assert_eq!(v["status"], "error");
    assert_eq!(v["error"], "agent exited without emitting complete");
    let back: AgentCompletePayload = serde_json::from_value(v).unwrap();
    assert_eq!(back.run_id, "r-7");
}

#[test]
fn test_agent_run_request_round_trip_camel_case() {
    let req = AgentRunRequest {
        text: "t".to_string(),
        selection_kind: "module".to_string(),
        selection_id: Some("m1".to_string()),
        path: vec!["m1".to_string()],
        lens: Lens::Rules,
        mode: AgentMode::Test,
        model: None,
    };
    let v: serde_json::Value = serde_json::to_value(&req).unwrap();
    assert_eq!(v["selectionKind"], "module");
    assert_eq!(v["selectionId"], "m1");
    assert_eq!(v["lens"], "rules");
    assert_eq!(v["mode"], "test");
    assert!(v.get("model").is_none(), "None should skip serialization");

    let back: AgentRunRequest = serde_json::from_value(v).unwrap();
    assert_eq!(back.selection_kind, "module");
    assert_eq!(back.mode, AgentMode::Test);
}
