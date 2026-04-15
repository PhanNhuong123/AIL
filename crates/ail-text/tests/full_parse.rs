use std::path::PathBuf;

use ail_contract::verify;
use ail_graph::{validate_graph, Pattern};
use ail_text::{parse, parse_directory};
use ail_types::type_check;

fn wallet_full_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wallet_full")
}

fn wallet_service_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ail-graph/tests/fixtures/wallet_service")
}

// ── Structure tests ─────────────────────────────────────────────────────────

#[test]
fn full_parse_directory_produces_correct_structure() {
    let graph = parse_directory(&wallet_full_path()).unwrap();

    // 1 Describe root + 2 Define + 2 Describe + 3 Do + 3 Let = 11 nodes
    assert_eq!(graph.node_count(), 11);

    // Root is a structural Describe container
    let root_id = graph.root_id().expect("should have a root");
    let root = graph.get_node(root_id).unwrap();
    assert_eq!(root.pattern, Pattern::Describe);
    assert_eq!(root.intent, "wallet_full");
    assert!(
        root.metadata.name.is_none(),
        "container node should have name = None"
    );

    // Root has 7 direct children (the 7 parsed files)
    let children = root.children.as_ref().expect("root should have children");
    assert_eq!(children.len(), 7);

    // Check child patterns (alphabetical order):
    // add_money.ail -> Do
    // deduct_money.ail -> Do
    // positive_amount.ail -> Define
    // transfer_money.ail -> Do
    // transfer_result.ail -> Describe
    // user.ail -> Describe
    // wallet_balance.ail -> Define
    let child_patterns: Vec<Pattern> = children
        .iter()
        .map(|id| graph.get_node(*id).unwrap().pattern.clone())
        .collect();
    assert_eq!(
        child_patterns,
        vec![
            Pattern::Do,
            Pattern::Do,
            Pattern::Define,
            Pattern::Do,
            Pattern::Describe,
            Pattern::Describe,
            Pattern::Define,
        ]
    );
}

// ── Pipeline tests ──────────────────────────────────────────────────────────

#[test]
fn full_parse_directory_validates() {
    let graph = parse_directory(&wallet_full_path()).unwrap();
    validate_graph(graph).unwrap_or_else(|errs| {
        panic!("parsed wallet_full should validate; errors: {errs:?}");
    });
}

#[test]
fn full_parse_directory_type_checks() {
    let graph = parse_directory(&wallet_full_path()).unwrap();
    let valid = validate_graph(graph).unwrap_or_else(|errs| {
        panic!("validation should pass; errors: {errs:?}");
    });
    type_check(valid, &[]).unwrap_or_else(|errs| {
        panic!("parsed wallet_full should type-check; errors: {errs:?}");
    });
}

#[test]
fn full_parse_directory_verifies() {
    let graph = parse_directory(&wallet_full_path()).unwrap();
    let valid = validate_graph(graph).unwrap_or_else(|errs| {
        panic!("validation should pass; errors: {errs:?}");
    });
    let typed = type_check(valid, &[]).unwrap_or_else(|errs| {
        panic!("type check should pass; errors: {errs:?}");
    });
    verify(typed).unwrap_or_else(|errs| {
        panic!("parsed wallet_full should verify; errors: {errs:?}");
    });
}

// ── Cross-file reference test ───────────────────────────────────────────────

#[test]
fn full_parse_cross_file_type_resolves() {
    // transfer_money.ail references WalletBalance (defined in wallet_balance.ail)
    // and PositiveAmount (defined in positive_amount.ail).
    // After parse_directory merges all files, type_check must resolve these
    // cross-file references without UndefinedType errors.
    let graph = parse_directory(&wallet_full_path()).unwrap();

    // Verify the Do node has param type refs that came from other files
    let do_node = graph
        .all_nodes()
        .find(|n| n.pattern == Pattern::Do)
        .expect("should have a Do node");
    assert_eq!(do_node.metadata.params.len(), 2);
    assert_eq!(do_node.metadata.params[0].type_ref, "WalletBalance");
    assert_eq!(do_node.metadata.params[1].type_ref, "PositiveAmount");

    // The type refs resolve because Define nodes exist in the same graph
    let valid = validate_graph(graph).unwrap_or_else(|errs| {
        panic!("validation should pass; errors: {errs:?}");
    });
    type_check(valid, &[]).unwrap_or_else(|errs| {
        panic!("cross-file type refs should resolve; errors: {errs:?}");
    });
}

// ── Existing fixture parse test ─────────────────────────────────────────────

#[test]
fn full_parse_each_existing_fixture_file_parses() {
    let fixture_root = wallet_service_path();
    assert!(
        fixture_root.exists(),
        "wallet_service fixture directory should exist at {:?}",
        fixture_root
    );

    let ail_files = collect_ail_files(&fixture_root);
    assert!(
        !ail_files.is_empty(),
        "should find .ail files in the fixture"
    );

    let mut parsed_count = 0;
    for file_path in &ail_files {
        let source = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", file_path.display()));

        let result = parse(&source);
        assert!(
            result.is_ok(),
            "parse failed for {}: {:?}",
            file_path.display(),
            result.as_ref().err()
        );
        parsed_count += 1;
    }

    // The wallet_service fixture has 30 .ail files
    assert_eq!(
        parsed_count, 30,
        "expected 30 .ail files in wallet_service fixture"
    );
}

fn collect_ail_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_ail_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("ail") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

// ── Edge case tests ─────────────────────────────────────────────────────────

#[test]
fn full_parse_empty_directory() {
    let tmp = std::env::temp_dir().join("ail_test_empty_dir");
    let _ = std::fs::create_dir_all(&tmp);
    // Ensure it's empty
    if let Ok(entries) = std::fs::read_dir(&tmp) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    let graph = parse_directory(&tmp).unwrap();
    assert_eq!(graph.node_count(), 1, "empty dir = 1 structural root");

    let root = graph.get_node(graph.root_id().unwrap()).unwrap();
    assert_eq!(root.pattern, Pattern::Describe);
    assert!(root.children.as_ref().unwrap().is_empty());

    // Structural root validates
    validate_graph(graph).unwrap_or_else(|errs| {
        panic!("empty directory graph should validate; errors: {errs:?}");
    });

    let _ = std::fs::remove_dir(&tmp);
}

#[test]
fn full_parse_nested_directory() {
    // Parse the wallet_service/concepts/ subdirectory which has
    // .ail files and a nested errors/ directory.
    let concepts_path = wallet_service_path().join("concepts");
    assert!(
        concepts_path.exists(),
        "concepts/ fixture should exist at {:?}",
        concepts_path
    );

    let graph = parse_directory(&concepts_path).unwrap();

    // concepts/ has: errors/ (subdir) + 6 .ail files = 7 children
    // errors/ has: 1 .ail file = 1 child
    // Plus container nodes: concepts + errors = 2
    // Total: 2 containers + 6 files + 1 file = 9 nodes
    let root_id = graph.root_id().unwrap();
    let root = graph.get_node(root_id).unwrap();
    assert_eq!(root.intent, "concepts");

    let children = root.children.as_ref().unwrap();
    // errors/ directory comes first (directories before files), then 6 .ail files
    assert_eq!(children.len(), 7);

    // First child should be the errors/ subdirectory (Describe container)
    let first_child = graph.get_node(children[0]).unwrap();
    assert_eq!(first_child.intent, "errors");
    assert_eq!(first_child.pattern, Pattern::Describe);

    // The errors/ subdirectory should have 1 child (insufficient_balance_error.ail)
    let error_children = first_child.children.as_ref().unwrap();
    assert_eq!(error_children.len(), 1);

    // Remaining 6 children should be parsed .ail files
    let file_patterns: Vec<Pattern> = children[1..]
        .iter()
        .map(|id| graph.get_node(*id).unwrap().pattern.clone())
        .collect();
    // Alphabetical: positive_amount (Define), transfer_result (Describe),
    // user (Describe), user_id (Define), user_status (Define), wallet_balance (Define)
    assert_eq!(
        file_patterns,
        vec![
            Pattern::Define,   // positive_amount
            Pattern::Describe, // transfer_result
            Pattern::Describe, // user
            Pattern::Define,   // user_id
            Pattern::Define,   // user_status
            Pattern::Define,   // wallet_balance
        ]
    );
}
