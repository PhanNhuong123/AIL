/// Tests for the assembler's Ed-edge resolution passes:
/// - `resolve_following_edges` wires Ed edges from implementing Do → template Do
/// - `resolve_using_edges` wires Ed edges from using-Do → shared-pattern Do
///
/// These tests use the public `parse()` function and inspect the resulting graph
/// for the presence (or absence) of `Ed` (diagonal) edges.
use ail_graph::Pattern;
use ail_text::parse;

// ── following clause: Ed-edge wiring ─────────────────────────────────────────

/// A Do node with `following <template>` gets an Ed edge to the named template Do.
#[test]
fn assembler_following_wires_ed_edge_to_template() {
    // Two top-level Do nodes: a template and an implementing node.
    let input = concat!(
        "do command flow template\n",
        "\n",
        "do save sender balance\n",
        "  following command flow template"
    );
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));

    // Find the implementing Do node.
    let impl_node = graph
        .all_nodes()
        .find(|n| n.intent == "save sender balance")
        .expect("implementing Do node not found");

    // It must have an outgoing Ed edge.
    let ed_targets = graph
        .outgoing_diagonal_refs_of(impl_node.id)
        .unwrap_or_default();
    assert!(
        !ed_targets.is_empty(),
        "following Do must have at least one outgoing Ed edge"
    );

    // The target must be the template Do.
    let template = graph
        .get_node(ed_targets[0])
        .expect("target node must exist");
    assert_eq!(
        template.intent, "command flow template",
        "Ed edge must point to the template node"
    );
}

/// A Do node without a `following` clause must have no Ed edges wired by the assembler.
#[test]
fn assembler_no_following_clause_no_ed_edge() {
    let input = "do save sender balance\n  from sender:User";
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));

    let node = graph
        .all_nodes()
        .find(|n| n.intent == "save sender balance")
        .expect("Do node not found");

    let ed_targets = graph.outgoing_diagonal_refs_of(node.id).unwrap_or_default();
    assert!(
        ed_targets.is_empty(),
        "Do without following clause must have no Ed edges from assembler"
    );
}

/// Referencing a template name that does not exist must return a parse error.
#[test]
fn assembler_following_unknown_template_returns_error() {
    let input = "do save sender balance\n  following nonexistent template";
    let result = parse(input);
    assert!(
        result.is_err(),
        "following a nonexistent template should fail parsing"
    );
    let Err(parse_err) = result else {
        panic!("expected Err");
    };
    let err = format!("{parse_err}");
    assert!(
        err.contains("nonexistent template") || err.contains("not found"),
        "error should mention the missing template name: {err}"
    );
}

// ── using clause: Ed-edge wiring ─────────────────────────────────────────────

/// A Do node with `using <pattern>` gets an Ed edge to the named shared-pattern Do.
#[test]
fn assembler_using_wires_ed_edge_to_pattern() {
    let input = concat!(
        "do save entity to database\n",
        "\n",
        "do save sender balance\n",
        "  using save entity to database\n",
        "    where entity is sender, entity_id is sender.id"
    );
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));

    let using_node = graph
        .all_nodes()
        .find(|n| n.intent == "save sender balance")
        .expect("using-Do node not found");

    // Must have Ed edge to shared pattern.
    let ed_targets = graph
        .outgoing_diagonal_refs_of(using_node.id)
        .unwrap_or_default();
    assert!(
        !ed_targets.is_empty(),
        "using-Do must have outgoing Ed edge to shared pattern"
    );

    let shared = graph
        .get_node(ed_targets[0])
        .expect("shared pattern node must exist");
    assert_eq!(shared.pattern, Pattern::Do);
    assert_eq!(
        shared.intent, "save entity to database",
        "Ed edge must point to the shared-pattern node"
    );
}

/// Metadata is populated: `using_pattern_name` and `using_params` are set.
#[test]
fn assembler_using_populates_metadata() {
    let input = concat!(
        "do save entity to database\n",
        "\n",
        "do save sender balance\n",
        "  using save entity to database\n",
        "    where entity is sender, entity_id is sender.id"
    );
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));

    let node = graph
        .all_nodes()
        .find(|n| n.intent == "save sender balance")
        .expect("using-Do node not found");

    assert_eq!(
        node.metadata.using_pattern_name.as_deref(),
        Some("save entity to database")
    );
    assert_eq!(node.metadata.using_params.len(), 2);
    assert_eq!(
        node.metadata.using_params[0],
        ("entity".to_owned(), "sender".to_owned())
    );
    assert_eq!(
        node.metadata.using_params[1],
        ("entity_id".to_owned(), "sender.id".to_owned())
    );
}

/// Referencing a pattern name that does not exist must return a parse error.
#[test]
fn assembler_using_unknown_pattern_returns_error() {
    let input = "do save sender balance\n  using nonexistent shared pattern";
    let result = parse(input);
    assert!(
        result.is_err(),
        "using a nonexistent pattern should fail parsing"
    );
    let Err(parse_err) = result else {
        panic!("expected Err");
    };
    let err = format!("{parse_err}");
    assert!(
        err.contains("nonexistent shared pattern") || err.contains("not found"),
        "error should mention the missing pattern name: {err}"
    );
}

/// A `using` clause with no `where` params parses successfully.
#[test]
fn assembler_using_no_params_is_valid() {
    let input = concat!(
        "do send notification\n",
        "\n",
        "do notify user\n",
        "  using send notification"
    );
    let graph = parse(input).unwrap_or_else(|e| panic!("parse failed: {e}"));

    let node = graph
        .all_nodes()
        .find(|n| n.intent == "notify user")
        .expect("using-Do node not found");

    assert_eq!(
        node.metadata.using_pattern_name.as_deref(),
        Some("send notification")
    );
    assert!(
        node.metadata.using_params.is_empty(),
        "no where clause → empty params"
    );

    // Ed edge must still be wired.
    let ed = graph.outgoing_diagonal_refs_of(node.id).unwrap_or_default();
    assert!(!ed.is_empty(), "Ed edge must be wired even without params");
}
