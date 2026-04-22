use ail_ui_bridge::types::graph_json::RelationJson;

/// Test 12: `RelationJson { style: Some("data") }` serializes with the
/// `style` field present and roundtrips correctly.
#[test]
fn test_relations_have_style() {
    let rel = RelationJson {
        from: "mod.fn_a".to_string(),
        to: "mod.TypeX".to_string(),
        label: String::new(),
        style: Some("data".to_string()),
    };

    let json = serde_json::to_string(&rel).expect("serialize");
    assert!(
        json.contains("\"style\":\"data\""),
        "serialized JSON must contain style:data, got: {json}"
    );

    let restored: RelationJson = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(rel, restored, "roundtrip must preserve equality");
    assert_eq!(restored.style.as_deref(), Some("data"));
}

// test_externals_separate removed — superseded by tests/externals_classification.rs
// which verifies wallet_service externals are empty and the field exists in JSON.
