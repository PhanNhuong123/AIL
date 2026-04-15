use std::collections::HashSet;

use ail_graph::{AilGraph, Node, NodeId, Pattern};

use crate::errors::EmitError;
use crate::python::function::emit_do_body;
use crate::types::{EmitConfig, ImportSet};

// ── Public entry point ────────────────────────────────────────────────────────

/// Emit a `Do` node that delegates its body to a shared-pattern via `using`.
///
/// Locates the shared-pattern `Do` node by following the outgoing `Ed`
/// (diagonal) edge set by the assembler, emits the pattern's body via
/// `emit_do_body`, then applies whole-word parameter substitution on the
/// resulting lines.
///
/// # v0.1 Limitation
///
/// Parameter substitution is text-level, not AST-level. Values should be
/// simple identifiers or dotted paths (e.g., `sender`, `sender.id`).
/// Multi-token expressions that contain one of the placeholder keys as a
/// substring outside a word boundary will not be substituted (that is safe).
/// Values that ARE multi-token may not round-trip through substitution if a
/// later substitution's key appears inside an earlier substitution's value.
pub(crate) fn emit_using_do(
    graph: &AilGraph,
    node: &Node,
    indent_level: usize,
    config: &EmitConfig,
    imports: &mut ImportSet,
) -> Result<Vec<String>, Vec<EmitError>> {
    // Caller (emit_do_function) only dispatches here when using_pattern_name is Some.
    let pattern_name = node
        .metadata
        .using_pattern_name
        .clone()
        .expect("emit_using_do called on a node without using_pattern_name");

    // Locate the template Do node via the outgoing Ed diagonal edge.
    let diagonal_refs = graph.outgoing_diagonal_refs_of(node.id).unwrap_or_default();

    // The v009 validation rule guarantees exactly one Ed edge for using-Do
    // nodes, but we handle the "not found" case defensively here.
    let template_id: NodeId = match diagonal_refs.first().copied() {
        Some(id) => id,
        None => {
            return Err(vec![EmitError::UsingDoMissingEdge { node_id: node.id }]);
        }
    };

    let template = match graph.get_node(template_id) {
        Ok(n) => n,
        Err(_) => {
            return Err(vec![EmitError::UsingDoUnresolvedPattern {
                node_id: node.id,
                pattern_name,
            }]);
        }
    };

    if template.pattern != Pattern::Do {
        return Err(vec![EmitError::UsingDoUnresolvedPattern {
            node_id: node.id,
            pattern_name,
        }]);
    }

    // Emit the shared-pattern body.
    // using-Do bodies contain no explicit Return nodes, so after_contracts is empty.
    let raw_lines = if let Some(children) = &template.children {
        emit_do_body(graph, children, indent_level, config, imports, &[])?
    } else {
        Vec::new()
    };

    // Apply parameter substitutions from the `where` clause.
    let substituted = apply_using_substitutions(raw_lines, &node.metadata.using_params);

    Ok(substituted)
}

// ── Substitution helpers ──────────────────────────────────────────────────────

/// Apply `using_params` substitutions to a list of code lines.
///
/// For each `(key, value)` pair, replaces all whole-word occurrences of `key`
/// in each line with `value`. Substitutions are applied in order.
pub(crate) fn apply_using_substitutions(
    lines: Vec<String>,
    params: &[(String, String)],
) -> Vec<String> {
    if params.is_empty() {
        return lines;
    }
    lines
        .into_iter()
        .map(|line| {
            let mut result = line;
            for (key, value) in params {
                result = replace_whole_word(&result, key, value);
            }
            result
        })
        .collect()
}

/// Replace all whole-word occurrences of `from` with `to` in `text`.
///
/// A "word boundary" is any position where the character immediately before or
/// after the match is not an ASCII alphanumeric character and not `_`.
pub(crate) fn replace_whole_word(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() || text.is_empty() {
        return text.to_owned();
    }

    let mut result = String::with_capacity(text.len() + to.len());
    let bytes = text.as_bytes();
    let from_bytes = from.as_bytes();
    let from_len = from_bytes.len();
    let text_len = bytes.len();

    let mut i = 0;
    while i < text_len {
        // Check if `from` starts at position `i`.
        if i + from_len <= text_len && bytes[i..i + from_len] == *from_bytes {
            // Check left boundary.
            let left_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            // Check right boundary.
            let right_ok = i + from_len >= text_len || !is_ident_char(bytes[i + from_len]);

            if left_ok && right_ok {
                result.push_str(to);
                i += from_len;
                continue;
            }
        }
        // No match at this position: copy the character.
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Returns true if `b` is an ASCII character that can appear inside an identifier.
#[inline]
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── Collect required phase names ──────────────────────────────────────────────

/// Collect the set of required phase names for a `Do` node that carries
/// `following` template references (outgoing `Ed` edges).
///
/// Used by `emit_do_function` to determine which top-level children need a
/// `# === [Phase: X] ===` comment injected before them.
pub(crate) fn collect_required_phases(graph: &AilGraph, do_node_id: NodeId) -> HashSet<String> {
    let template_ids = graph
        .outgoing_diagonal_refs_of(do_node_id)
        .unwrap_or_default();

    template_ids
        .iter()
        .flat_map(|&tid| graph.children_of(tid).unwrap_or_default())
        .filter_map(|cid| {
            graph
                .get_node(cid)
                .ok()
                .and_then(|n| n.metadata.name.clone())
        })
        .collect()
}
