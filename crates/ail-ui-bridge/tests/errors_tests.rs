use ail_ui_bridge::errors::BridgeError;
use ail_ui_bridge::pipeline::load_verified_from_path;
use std::path::Path;

/// Test 15: loading a non-existent path returns a BridgeError.
#[test]
fn test_error_on_invalid_path() {
    let result = load_verified_from_path(Path::new("/nonexistent_path_that_does_not_exist"));

    assert!(result.is_err(), "expected Err for non-existent path");

    let err = result.unwrap_err();
    // Accept either ProjectNotFound or PipelineError { stage: "parse", .. }
    let is_expected = matches!(
        &err,
        BridgeError::ProjectNotFound { .. } | BridgeError::PipelineError { .. }
    );
    assert!(is_expected, "unexpected error variant: {err:?}");

    // Verify error serializes to a structured code+detail shape.
    let json = serde_json::to_value(&err).expect("BridgeError must serialize");
    assert!(
        json.get("code").is_some(),
        "serialized error must have 'code' field"
    );
}
