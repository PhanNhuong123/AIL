//! Handlers for `ail.write` and `ail.patch` MCP tools (Phase 11.1).
//!
//! These tools let AI agents create and update graph nodes directly instead of
//! writing `.ail` text files and re-parsing. Every mutation validates inputs,
//! detects obvious Ed edges from type references in expressions, and returns
//! structured results.

use ail_graph::{
    AilGraph, Contract, ContractKind, EdgeKind, Expression, GraphBackend, Node, NodeId,
    NodeMetadata, Pattern,
};

use crate::types::tool_io::{
    AutoEdgeOutput, ContractInput, PatchInput, PatchOutput, WriteInput, WriteOutput,
};

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Parse a snake_case pattern string (e.g. `"do"`, `"define"`) into a `Pattern`.
///
/// Uses the same serde JSON technique as `ail-db`'s `pattern_from_sql`.
fn parse_pattern_str(s: &str) -> Result<Pattern, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("invalid pattern: \"{s}\""))
}

/// Parse a snake_case contract kind string into a `ContractKind`.
fn parse_contract_kind_str(s: &str) -> Result<ContractKind, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("invalid contract kind: \"{s}\""))
}

/// Parse a string as a `NodeId` (UUID).
pub(crate) fn parse_node_id(s: &str) -> Result<NodeId, String> {
    s.parse::<NodeId>()
        .map_err(|_| format!("invalid node id: \"{s}\""))
}

/// Build the intent path from a node up to the root.
///
/// Returns `["root intent", "child intent", ..., "this node intent"]`.
pub(crate) fn compute_node_path(graph: &AilGraph, node_id: NodeId) -> Vec<String> {
    let mut path = Vec::new();
    // Collect ancestors bottom-up, then reverse.
    let mut current = node_id;
    loop {
        if let Ok(node) = graph.get_node(current) {
            let label = node
                .metadata
                .name
                .as_deref()
                .unwrap_or(node.intent.as_str());
            path.push(label.to_string());
        }
        match graph.parent_of(current) {
            Ok(Some(parent)) => current = parent,
            _ => break,
        }
    }
    path.reverse();
    path
}

/// Extract PascalCase identifiers from an expression string.
///
/// A PascalCase word starts with an uppercase letter and contains at least one
/// lowercase letter (to distinguish from ALL_CAPS constants).
fn extract_pascal_case_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_uppercase() {
            let mut word = String::new();
            let mut has_lower = false;
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    if c.is_ascii_lowercase() {
                        has_lower = true;
                    }
                    word.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if has_lower && word.len() > 1 {
                words.push(word);
            }
        } else {
            chars.next();
        }
    }
    words
}

/// Extract tokens that follow a bare keyword like `do` or `raise`.
///
/// Matches only when the keyword appears as a whole word (preceded by start of
/// text, whitespace, or punctuation). The returned tokens are the next
/// identifier-like sequence of alphanumerics/underscore.
fn extract_tokens_after_keyword(text: &str, keyword: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + keyword.len() <= bytes.len() {
        let prev_is_boundary = i == 0 || !is_ident_char(bytes[i - 1]);
        let window = &bytes[i..i + keyword.len()];
        let next_is_boundary =
            i + keyword.len() == bytes.len() || !is_ident_char(bytes[i + keyword.len()]);
        if prev_is_boundary && next_is_boundary && window.eq_ignore_ascii_case(keyword.as_bytes()) {
            let mut j = i + keyword.len();
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let start = j;
            while j < bytes.len() && is_ident_char(bytes[j]) {
                j += 1;
            }
            if j > start {
                if let Ok(tok) = std::str::from_utf8(&bytes[start..j]) {
                    out.push(tok.to_string());
                }
            }
            i = j.max(i + keyword.len());
        } else {
            i += 1;
        }
    }
    out
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Scan a node's expression and intent for references and return the Ed edges
/// that should be created. Handles PascalCase type/error/function refs,
/// snake_case `do <name>` function calls, `raise <Name>` / `otherwise raise
/// <Name>` keywords, and `follows_template` from `metadata.following_template_name`.
pub(crate) fn detect_auto_edges(graph: &AilGraph, node_id: NodeId) -> Vec<AutoEdgeOutput> {
    let node = match graph.get_node(node_id) {
        Ok(n) => n,
        Err(_) => return vec![],
    };

    let mut edges: Vec<AutoEdgeOutput> = Vec::new();
    let mut seen: std::collections::HashSet<(NodeId, &'static str)> =
        std::collections::HashSet::new();

    let push_edge =
        |target_id: NodeId,
         label: &'static str,
         edges: &mut Vec<AutoEdgeOutput>,
         seen: &mut std::collections::HashSet<(NodeId, &'static str)>| {
            if target_id == node_id {
                return;
            }
            if !seen.insert((target_id, label)) {
                return;
            }
            edges.push(AutoEdgeOutput {
                kind: "ed".into(),
                target: target_id.to_string(),
                label: label.into(),
            });
        };

    // 1. `follows_template` from metadata — authoritative template reference.
    if let Some(template_name) = &node.metadata.following_template_name {
        if let Ok(targets) = GraphBackend::find_by_name(graph, template_name) {
            for target_id in targets {
                push_edge(target_id, "follows_template", &mut edges, &mut seen);
            }
        }
    }

    // 2. Collect source texts: expression + intent.
    let mut sources: Vec<&str> = Vec::with_capacity(2);
    if let Some(expr) = &node.expression {
        sources.push(expr.0.as_str());
    }
    sources.push(node.intent.as_str());

    for text in &sources {
        // 2a. PascalCase references — classify by target pattern.
        for name in extract_pascal_case_words(text) {
            if let Ok(targets) = GraphBackend::find_by_name(graph, &name) {
                for target_id in targets {
                    if let Ok(Some(target_node)) = GraphBackend::get_node(graph, target_id) {
                        let label: &'static str = match target_node.pattern {
                            Pattern::Define | Pattern::Describe => "uses_type",
                            Pattern::Error => "raises",
                            Pattern::Do => "calls",
                            _ => continue,
                        };
                        push_edge(target_id, label, &mut edges, &mut seen);
                    }
                }
            }
        }

        // 2b. Snake_case `do <name>` function subcalls.
        for name in extract_tokens_after_keyword(text, "do") {
            if let Ok(targets) = GraphBackend::find_by_name(graph, &name) {
                for target_id in targets {
                    if let Ok(Some(target_node)) = GraphBackend::get_node(graph, target_id) {
                        if target_node.pattern == Pattern::Do {
                            push_edge(target_id, "calls", &mut edges, &mut seen);
                        }
                    }
                }
            }
        }

        // 2c. `raise <Name>` / `otherwise raise <Name>` — Error references.
        for name in extract_tokens_after_keyword(text, "raise") {
            if let Ok(targets) = GraphBackend::find_by_name(graph, &name) {
                for target_id in targets {
                    if let Ok(Some(target_node)) = GraphBackend::get_node(graph, target_id) {
                        if target_node.pattern == Pattern::Error {
                            push_edge(target_id, "raises", &mut edges, &mut seen);
                        }
                    }
                }
            }
        }
    }

    edges
}

/// Convert `ContractInput` DTOs into graph `Contract` values.
fn parse_contracts(inputs: &[ContractInput]) -> Result<Vec<Contract>, String> {
    inputs
        .iter()
        .map(|c| {
            let kind = parse_contract_kind_str(&c.kind)?;
            Ok(Contract {
                kind,
                expression: Expression(c.expression.clone()),
            })
        })
        .collect()
}

/// Insert a new node as a child of `parent_id` at the given position in the
/// Eh sibling chain. If `position` is `None` or past the end, the node is
/// appended as the last child.
pub(crate) fn insert_at_position(
    graph: &mut AilGraph,
    parent_id: NodeId,
    new_id: NodeId,
    position: Option<usize>,
) -> Result<(), String> {
    // Exclude new_id from the sibling list — the Ev edge was already added, so
    // children() includes the new node. We only want the pre-existing siblings.
    let siblings: Vec<NodeId> = GraphBackend::children(graph, parent_id)
        .map_err(|e| format!("failed to read children: {e}"))?
        .into_iter()
        .filter(|&id| id != new_id)
        .collect();

    if siblings.is_empty() {
        // First child — no Eh edges needed.
        return Ok(());
    }

    let pos = position.unwrap_or(siblings.len());

    if pos >= siblings.len() {
        // Append: Eh from current last sibling → new node.
        let last = siblings[siblings.len() - 1];
        graph
            .add_edge(last, new_id, EdgeKind::Eh)
            .map_err(|e| format!("failed to add Eh edge: {e}"))?;
    } else if pos == 0 {
        // Prepend: Eh from new node → current first sibling.
        let first = siblings[0];
        graph
            .add_edge(new_id, first, EdgeKind::Eh)
            .map_err(|e| format!("failed to add Eh edge: {e}"))?;
    } else {
        // Insert in middle: prev → new → next.
        let prev = siblings[pos - 1];
        let next = siblings[pos];
        // Remove old link prev → next.
        GraphBackend::remove_edge_by_kind(graph, prev, next, EdgeKind::Eh)
            .map_err(|e| format!("failed to remove Eh edge: {e}"))?;
        // Add prev → new.
        graph
            .add_edge(prev, new_id, EdgeKind::Eh)
            .map_err(|e| format!("failed to add Eh edge: {e}"))?;
        // Add new → next.
        graph
            .add_edge(new_id, next, EdgeKind::Eh)
            .map_err(|e| format!("failed to add Eh edge: {e}"))?;
    }

    Ok(())
}

// ── ail.write ────────────────────────────────────────────────────────────────

/// Create a new node under `parent_id` with the given pattern, intent,
/// optional expression, position, and contracts. Returns a structured result
/// with the new node ID, depth, path, and any auto-detected Ed edges.
pub(crate) fn run_write(graph: &mut AilGraph, input: &WriteInput) -> Result<WriteOutput, String> {
    let mut warnings = Vec::new();

    // 1. Validate inputs [11.1-A].
    if input.intent.trim().is_empty() {
        return Err("intent must not be empty".into());
    }
    let pattern = parse_pattern_str(&input.pattern)?;
    let parent_id = parse_node_id(&input.parent_id)?;

    // 2. Verify parent exists.
    GraphBackend::get_node(graph, parent_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("parent node not found: {}", input.parent_id))?;

    // 3. Build the node.
    let node_id = NodeId::new();
    let mut node = Node::new(node_id, input.intent.clone(), pattern);
    if let Some(expr) = &input.expression {
        node.expression = Some(Expression(expr.clone()));
    }

    // 4. Parse and attach contracts.
    if let Some(contract_inputs) = &input.contracts {
        node.contracts = parse_contracts(contract_inputs)?;
    }

    // 4b. Parse and attach metadata (shallow-merge into default).
    if let Some(meta_value) = &input.metadata {
        if let Some(obj) = meta_value.as_object() {
            let mut current = serde_json::to_value(&node.metadata)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            if let Some(current_obj) = current.as_object_mut() {
                for (k, v) in obj {
                    current_obj.insert(k.clone(), v.clone());
                }
            }
            match serde_json::from_value::<NodeMetadata>(current) {
                Ok(merged) => node.metadata = merged,
                Err(_) => {
                    warnings.push("metadata merge failed: incompatible fields ignored".into())
                }
            }
        }
    }

    // 5. Insert node and Ev edge.
    graph
        .add_node(node)
        .map_err(|e| format!("failed to add node: {e}"))?;
    graph
        .add_edge(parent_id, node_id, EdgeKind::Ev)
        .map_err(|e| format!("failed to add Ev edge: {e}"))?;

    // 6. Handle sibling position [11.1-B].
    insert_at_position(graph, parent_id, node_id, input.position)?;

    // 7. Auto-detect Ed edges.
    let auto_edges = detect_auto_edges(graph, node_id);
    for edge in &auto_edges {
        let target_id = parse_node_id(&edge.target)?;
        if let Err(e) = graph.add_edge(node_id, target_id, EdgeKind::Ed) {
            warnings.push(format!("auto-edge to {} skipped: {e}", edge.target));
        }
    }

    // 8. Compute depth and path.
    let depth = GraphBackend::depth(graph, node_id).unwrap_or(0);
    let path = compute_node_path(graph, node_id);

    Ok(WriteOutput {
        status: "created".into(),
        node_id: node_id.to_string(),
        depth,
        path,
        auto_edges,
        cic_invalidated: 0, // AilGraph has no CIC cache
        warnings,
    })
}

// ── ail.patch ────────────────────────────────────────────────────────────────

/// Update fields on an existing node. Only the fields present in
/// `input.fields` are changed. Returns a structured result with the list of
/// changed fields and any Ed edge changes from re-detection.
pub(crate) fn run_patch(graph: &mut AilGraph, input: &PatchInput) -> Result<PatchOutput, String> {
    let mut warnings = Vec::new();
    let mut changed_fields = Vec::new();

    // 1. Parse and verify node exists.
    let node_id = parse_node_id(&input.node_id)?;
    let mut node = GraphBackend::get_node(graph, node_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("node not found: {}", input.node_id))?;

    // 2. Snapshot existing outgoing Ed edges for diff.
    let old_auto_edges = detect_auto_edges(graph, node_id);

    // 3. Apply each provided field.
    let fields = &input.fields;

    if let Some(intent) = &fields.intent {
        if intent.trim().is_empty() {
            return Err("intent must not be empty".into());
        }
        if node.intent != *intent {
            node.intent = intent.clone();
            changed_fields.push("intent".into());
        }
    }

    if let Some(expr) = &fields.expression {
        let new_expr = Expression(expr.clone());
        if node.expression.as_ref() != Some(&new_expr) {
            node.expression = Some(new_expr);
            changed_fields.push("expression".into());
        }
    }

    if let Some(pattern_str) = &fields.pattern {
        let new_pattern = parse_pattern_str(pattern_str)?;
        if node.pattern != new_pattern {
            node.pattern = new_pattern;
            changed_fields.push("pattern".into());
        }
    }

    if let Some(contract_inputs) = &fields.contracts {
        let new_contracts = parse_contracts(contract_inputs)?;
        node.contracts = new_contracts;
        changed_fields.push("contracts".into());
    }

    if let Some(meta_value) = &fields.metadata {
        if let Some(obj) = meta_value.as_object() {
            // Shallow merge: serialize current metadata to JSON, merge keys, deserialize back.
            let mut current = serde_json::to_value(&node.metadata)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            if let Some(current_obj) = current.as_object_mut() {
                for (k, v) in obj {
                    current_obj.insert(k.clone(), v.clone());
                }
            }
            if let Ok(merged) = serde_json::from_value::<NodeMetadata>(current) {
                node.metadata = merged;
                changed_fields.push("metadata".into());
            } else {
                warnings.push("metadata merge failed: incompatible fields ignored".into());
            }
        }
    }

    // 4. Update the node in the graph.
    if !changed_fields.is_empty() {
        GraphBackend::update_node(graph, node_id, node)
            .map_err(|e| format!("failed to update node: {e}"))?;
    }

    // 5. Re-detect auto edges and compute diff.
    let new_auto_edges = detect_auto_edges(graph, node_id);

    let auto_edges_added: Vec<AutoEdgeOutput> = new_auto_edges
        .iter()
        .filter(|e| !old_auto_edges.contains(e))
        .cloned()
        .collect();
    let auto_edges_removed: Vec<AutoEdgeOutput> = old_auto_edges
        .iter()
        .filter(|e| !new_auto_edges.contains(e))
        .cloned()
        .collect();

    // 6. Apply edge diff to the graph.
    for edge in &auto_edges_added {
        let target_id = parse_node_id(&edge.target)?;
        if let Err(e) = graph.add_edge(node_id, target_id, EdgeKind::Ed) {
            warnings.push(format!("auto-edge to {} skipped: {e}", edge.target));
        }
    }
    for edge in &auto_edges_removed {
        let target_id = parse_node_id(&edge.target)?;
        // Best-effort removal — edge might already be gone.
        let _ = GraphBackend::remove_edge_by_kind(graph, node_id, target_id, EdgeKind::Ed);
    }

    Ok(PatchOutput {
        status: "updated".into(),
        node_id: node_id.to_string(),
        changed_fields,
        auto_edges_added,
        auto_edges_removed,
        cic_invalidated: 0,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_extraction_basic() {
        let words = extract_pascal_case_words("from sender:User -> TransferResult");
        assert!(words.contains(&"User".to_string()));
        assert!(words.contains(&"TransferResult".to_string()));
        assert!(!words.contains(&"from".to_string()));
        assert!(!words.contains(&"sender".to_string()));
    }

    #[test]
    fn pascal_case_ignores_all_caps() {
        let words = extract_pascal_case_words("MAX_SIZE and User");
        // MAX_SIZE starts uppercase but has no lowercase → excluded
        assert!(!words.iter().any(|w| w.starts_with("MAX")));
        assert!(words.contains(&"User".to_string()));
    }

    #[test]
    fn parse_pattern_valid() {
        assert_eq!(parse_pattern_str("do").unwrap(), Pattern::Do);
        assert_eq!(parse_pattern_str("define").unwrap(), Pattern::Define);
        assert_eq!(parse_pattern_str("check").unwrap(), Pattern::Check);
    }

    #[test]
    fn parse_pattern_invalid() {
        assert!(parse_pattern_str("bogus").is_err());
        assert!(parse_pattern_str("").is_err());
    }

    #[test]
    fn parse_contract_kind_valid() {
        assert_eq!(
            parse_contract_kind_str("before").unwrap(),
            ContractKind::Before
        );
        assert_eq!(
            parse_contract_kind_str("after").unwrap(),
            ContractKind::After
        );
        assert_eq!(
            parse_contract_kind_str("always").unwrap(),
            ContractKind::Always
        );
    }
}
