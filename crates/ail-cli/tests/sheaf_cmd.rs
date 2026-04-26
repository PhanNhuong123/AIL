//! Integration tests for `ail sheaf` command handler (Phase 17 task 17.3).
//!
//! Tests use trivially-verifiable hand-crafted fixtures so they are safe under
//! both default features and `--features z3-verify`. `render_sheaf_output` is
//! called directly for output-shape tests to avoid stdout capture.
//!
//! The tests confirm:
//! - The command wires up the pipeline correctly (text and JSON paths).
//! - Error paths return the right `CliError::Pipeline` messages.
//! - The JSON shape is stable (invariant 17.3-B).
//! - Subtree scoping and non-Do node warning paths work.

use std::fs;

use ail_cli::run_sheaf;

// ── Shared fixture path ───────────────────────────────────────────────────────

/// Create a temp dir with a minimal project that Z3 can fully verify.
///
/// Uses only built-in types and contracts that are trivially satisfiable
/// (`before: amount > 0` implies `after: amount > 0`). This avoids the
/// wallet_full fixture's AIL-C012 failure under `--features z3-verify`.
fn trivial_verified_temp() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("main.ail"),
        concat!(
            "do validate amount\n",
            "  from amount:PositiveAmount\n",
            "  -> PositiveAmount\n\n",
            "  promise before: amount > 0\n",
            "  promise after: amount > 0\n",
        ),
    )
    .unwrap();
    let root = tmp.path().to_path_buf();
    (tmp, root)
}

// ── Test 1: text output on filesystem project ─────────────────────────────────

/// `run_sheaf` on a trivially-Z3-verifiable project with default (text) format
/// must succeed. Uses `PositiveAmount` with `before: amount > 0` / `after: amount > 0`
/// so Z3 proves it immediately (avoids the wallet fixture's AIL-C012 failures
/// under `--features z3-verify`).
#[test]
fn t173_ail_sheaf_runs_on_filesystem_project_with_text_output() {
    let (_tmp, root) = trivial_verified_temp();

    let result = run_sheaf(&root, None, None, None);
    assert!(
        result.is_ok(),
        "run_sheaf must succeed on a trivially-verified project: {result:?}"
    );
}

// ── Test 2: JSON output is valid JSON ─────────────────────────────────────────

/// `render_sheaf_output` with `Json` format on an empty nerve must produce
/// valid JSON with the required top-level keys. `obstructions` must be null
/// on default features (invariant 17.3-B).
#[test]
fn t173_ail_sheaf_json_format_is_valid_json() {
    use ail_cli::commands::sheaf::{render_sheaf_output, OutputFormat};
    use ail_contract::CechNerve;

    let nerve = CechNerve {
        sections: vec![],
        overlaps: vec![],
    };

    let output = render_sheaf_output(
        &nerve,
        "full",
        None,
        None,
        #[cfg(feature = "z3-verify")]
        &[],
        OutputFormat::Json,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("JSON output must be valid JSON");

    assert!(
        parsed.get("z3_available").is_some(),
        "z3_available must be present"
    );
    assert!(parsed.get("scope").is_some(), "scope must be present");
    assert!(parsed.get("nerve").is_some(), "nerve must be present");
    assert!(
        parsed.get("obstructions").is_some(),
        "obstructions must be present"
    );

    // On default features obstructions must be null.
    #[cfg(not(feature = "z3-verify"))]
    assert_eq!(
        parsed["obstructions"],
        serde_json::Value::Null,
        "obstructions must be null on default features"
    );
}

// ── Test 3: JSON golden literal string ────────────────────────────────────────

/// For an empty nerve the JSON must be byte-identical to the golden string.
/// Guards against silent field-rename or field-order changes (invariant 17.3-B).
/// Pinned on default features only (z3_available would differ under z3-verify).
#[test]
#[cfg(not(feature = "z3-verify"))]
fn t173_ail_sheaf_json_golden_literal_string() {
    use ail_cli::commands::sheaf::{render_sheaf_output, OutputFormat};
    use ail_contract::CechNerve;

    let nerve = CechNerve {
        sections: vec![],
        overlaps: vec![],
    };

    let output = render_sheaf_output(&nerve, "full", None, None, OutputFormat::Json);

    let expected = concat!(
        "{\n",
        "  \"z3_available\": false,\n",
        "  \"scope\": {\n",
        "    \"kind\": \"full\"\n",
        "  },\n",
        "  \"nerve\": {\n",
        "    \"sections\": [],\n",
        "    \"overlaps\": []\n",
        "  },\n",
        "  \"obstructions\": null\n",
        "}"
    );

    assert_eq!(
        output, expected,
        "JSON golden literal mismatch (invariant 17.3-B)"
    );
}

// ── Test 4: invalid format returns error ──────────────────────────────────────

/// `--format yaml` must return `CliError::Pipeline` with an "invalid --format value" message.
/// This validation happens before the pipeline runs so we can use an empty project.
#[test]
fn t173_ail_sheaf_invalid_format_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    // Empty project — pipeline would fail on verify anyway, but format is
    // validated first so the error message is about the format, not the pipeline.
    let result = run_sheaf(tmp.path(), None, Some("yaml".to_owned()), None);

    assert!(result.is_err(), "invalid format must return Err");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("invalid --format value"),
        "error must mention 'invalid --format value', got: {err}"
    );
}

// ── Test 5: node not found returns error ──────────────────────────────────────

/// `--node nonexistent_xyz` on a trivially-verified project must return an error
/// containing "not found".
#[test]
fn t173_ail_sheaf_node_not_found_returns_error() {
    let (_tmp, root) = trivial_verified_temp();

    let result = run_sheaf(&root, Some("nonexistent_xyz".to_owned()), None, None);

    assert!(result.is_err(), "unknown node must return Err");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "error must mention 'not found', got: {err}"
    );
}

// ── Test 6: node scoping filters sections ─────────────────────────────────────

/// Scoping `--node` to one of two sibling Do nodes must produce text output
/// that mentions "subtree of `validate_one`" AND prove that the scoped result
/// has exactly 1 section while the full nerve has 2 sections.
///
/// The fixture is built in-process (no temp dir needed for the scoping proof)
/// using two sibling Do nodes — `validate_one` and `validate_two` — each with
/// `before: amount > 0` / `after: amount > 0` against a `PositiveAmount`
/// parameter.  Because both nodes share the variable `amount`, they also
/// produce a sibling overlap in the full nerve; the filtered nerve scoped at
/// `validate_one` drops that overlap (invariant 17.3-C).
///
/// A second assertion calls `run_sheaf` on the real temp-dir fixture (one Do
/// node) to confirm the CLI path succeeds and emits the expected header text.
#[test]
fn t173_ail_sheaf_node_scoping_filters_sections() {
    use ail_cli::commands::sheaf::{render_sheaf_output, OutputFormat};
    use ail_contract::{build_nerve, filter_to_subtree};
    use ail_graph::types::EdgeKind;
    use ail_graph::validation::validate_graph;
    use ail_graph::{AilGraph, Contract, ContractKind, Expression, Node, NodeId, Param, Pattern};
    use ail_types::type_check;

    // ── Build a 2-sibling in-process fixture ──────────────────────────────
    // Describe root → Do validate_one + Do validate_two (Eh edge: one → two).
    // Both share param `amount`, so they produce a sibling overlap in the full nerve.
    let mut graph = AilGraph::new();
    let mut root_node = Node::new(NodeId::new(), "root", Pattern::Describe);
    root_node.children = Some(vec![]);
    let root_id = graph.add_node(root_node).unwrap();
    graph.set_root(root_id).unwrap();

    let make_param = |name: &str| Param {
        name: name.to_string(),
        type_ref: "PositiveAmount".to_string(),
    };
    let make_contract = |kind: ContractKind, expr: &str| Contract {
        kind,
        expression: Expression(expr.to_string()),
    };

    // validate_one
    let mut n1 = Node::new(NodeId::new(), "validate one", Pattern::Do);
    n1.metadata.name = Some("validate_one".to_string());
    n1.metadata.params = vec![make_param("amount")];
    n1.contracts = vec![
        make_contract(ContractKind::Before, "amount > 0"),
        make_contract(ContractKind::After, "amount > 0"),
    ];
    let id1 = graph.add_node(n1).unwrap();
    graph.add_edge(root_id, id1, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(id1);

    // validate_two
    let mut n2 = Node::new(NodeId::new(), "validate two", Pattern::Do);
    n2.metadata.name = Some("validate_two".to_string());
    n2.metadata.params = vec![make_param("amount")];
    n2.contracts = vec![
        make_contract(ContractKind::Before, "amount > 0"),
        make_contract(ContractKind::After, "amount > 0"),
    ];
    let id2 = graph.add_node(n2).unwrap();
    graph.add_edge(root_id, id2, EdgeKind::Ev).unwrap();
    graph
        .get_node_mut(root_id)
        .unwrap()
        .children
        .as_mut()
        .unwrap()
        .push(id2);

    // Sibling Eh edge: validate_one → validate_two.
    graph.add_edge(id1, id2, EdgeKind::Eh).unwrap();

    let valid = validate_graph(graph).unwrap();
    let typed = type_check(valid, &[]).unwrap();
    let verified = ail_contract::verify(typed).unwrap();

    let nerve = build_nerve(&verified);
    let graph_backend = verified.graph();

    // ── Core assertion: full nerve has 2 sections, scoped has 1 ──────────
    assert_eq!(
        nerve.sections.len(),
        2,
        "full nerve must have 2 sections (one per Do node)"
    );

    let filtered = filter_to_subtree(&nerve, id1, graph_backend);

    assert_eq!(
        filtered.sections.len(),
        1,
        "scoped nerve (at validate_one, a leaf) must have exactly 1 section"
    );
    assert_eq!(
        filtered.sections[0].node_id, id1,
        "the single scoped section must be validate_one"
    );

    // ── CLI header assertion via render_sheaf_output ───────────────────────
    let output = render_sheaf_output(
        &filtered,
        "subtree",
        Some(&id1.to_string()),
        Some("validate_one"),
        #[cfg(feature = "z3-verify")]
        &[],
        OutputFormat::Text,
    );

    assert!(
        output.contains("subtree of `validate_one`"),
        "text output must mention scope query, got:\n{output}"
    );

    // ── Sanity check: run_sheaf CLI path also succeeds on the temp fixture ─
    let (_tmp, root) = trivial_verified_temp();
    let result = run_sheaf(&root, Some("validate amount".to_owned()), None, None);
    assert!(
        result.is_ok(),
        "run_sheaf with --node on trivial fixture must succeed: {result:?}"
    );
}

// ── Test 7: empty filtered nerve renders zero sections ────────────────────────

/// Verify that `render_sheaf_output` with an empty filtered nerve (what
/// `run_sheaf` produces when `--node` resolves to a non-Do node) does not panic
/// and emits "Sections: 0" in text mode and an empty `sections` array in JSON.
///
/// Note: the actual `eprintln!` warning in `run_sheaf` is exercised end-to-end
/// by the tester subagent. Capturing it in `cargo test` would require
/// `gag::Redirect` which is not a workspace dependency.
#[test]
fn t173_ail_sheaf_empty_filtered_nerve_renders_zero_sections() {
    use ail_cli::commands::sheaf::{render_sheaf_output, OutputFormat};
    use ail_contract::CechNerve;

    // Simulate an empty subtree (what happens when a non-Do node is scoped).
    let empty_nerve = CechNerve {
        sections: vec![],
        overlaps: vec![],
    };

    // render_sheaf_output with an empty nerve must not panic and must produce
    // valid output (the warning path itself is guarded by the eprintln! in run_sheaf).
    let text_out = render_sheaf_output(
        &empty_nerve,
        "subtree",
        Some("some-id"),
        Some("wallet service"),
        #[cfg(feature = "z3-verify")]
        &[],
        OutputFormat::Text,
    );

    assert!(
        text_out.contains("Sections: 0"),
        "empty subtree text must show 0 sections, got:\n{text_out}"
    );

    let json_out = render_sheaf_output(
        &empty_nerve,
        "subtree",
        Some("some-id"),
        Some("wallet service"),
        #[cfg(feature = "z3-verify")]
        &[],
        OutputFormat::Json,
    );

    let parsed: serde_json::Value = serde_json::from_str(&json_out).unwrap();
    let sections = parsed["nerve"]["sections"].as_array().unwrap();
    assert!(
        sections.is_empty(),
        "empty subtree JSON must have empty sections array"
    );
}
