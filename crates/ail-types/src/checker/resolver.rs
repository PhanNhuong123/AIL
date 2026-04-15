use ail_graph::{GraphBackend, NodeId, Pattern};

use crate::builtins::BuiltinSemanticType;

/// A fully-resolved type reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedType {
    Base(BaseKind),
    Builtin(BuiltinSemanticType),
    UserDefined { node_id: NodeId },
}

/// The primitive base types in the AIL type system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseKind {
    Integer,
    Number,
    Text,
    Boolean,
    Bytes,
    Timestamp,
    Record,
    List,
    Option,
    /// `void` — used by `Do` nodes that produce no value.
    Void,
}

/// Resolve a raw `type_ref` string to a [`ResolvedType`].
///
/// Resolution order (matches the code top-to-bottom):
/// 1. Handle parametric types (`"list<T>"`, `"option<T>"`) by stripping the
///    wrapper and resolving the inner type `T` recursively.
/// 2. Strip any namespace prefix (e.g. `"billing.Invoice"` → `"Invoice"`).
///    Full namespace resolution is owned by Phase 4 (parser + index).
///    TODO(phase-4): replace flat lookup with scoped index resolution.
/// 3. Match against base-type keywords (case-insensitive).
/// 4. Try [`BuiltinSemanticType::from_name`].
/// 5. Scan graph for a `Define` / `Describe` / `Error` node whose
///    `metadata.name` matches.
///
/// Returns `None` if the name is not recognised in any category.
pub(crate) fn resolve_type_ref(type_ref: &str, graph: &dyn GraphBackend) -> Option<ResolvedType> {
    let trimmed = type_ref.trim();

    // ── Parametric types: list<T> / option<T> ───────────────────────────────
    if let Some(inner) = strip_parametric_wrapper(trimmed, "list<") {
        // The wrapper resolves as List; we also validate the inner type.
        // Return None only if the inner type itself is unknown.
        resolve_type_ref(inner, graph)?;
        return Some(ResolvedType::Base(BaseKind::List));
    }
    if let Some(inner) = strip_parametric_wrapper(trimmed, "option<") {
        resolve_type_ref(inner, graph)?;
        return Some(ResolvedType::Base(BaseKind::Option));
    }

    // Strip namespace prefix: "billing.Invoice" → "Invoice"
    // TODO(phase-4): scoped lookup when the index resolver is ready.
    let name = trimmed.rsplit('.').next().unwrap_or(trimmed);

    // ── Base keywords ────────────────────────────────────────────────────────
    if let Some(base) = resolve_base_kind(name) {
        return Some(ResolvedType::Base(base));
    }

    // ── Builtin semantic types ───────────────────────────────────────────────
    if let Some(builtin) = BuiltinSemanticType::from_name(name) {
        return Some(ResolvedType::Builtin(builtin));
    }

    // ── User-defined graph nodes ─────────────────────────────────────────────
    find_type_node_by_name(graph, name).map(|node_id| ResolvedType::UserDefined { node_id })
}

/// Match a name against AIL base-type keywords (case-insensitive).
fn resolve_base_kind(name: &str) -> Option<BaseKind> {
    match name.to_ascii_lowercase().as_str() {
        "integer" => Some(BaseKind::Integer),
        "number" => Some(BaseKind::Number),
        "text" => Some(BaseKind::Text),
        "boolean" => Some(BaseKind::Boolean),
        "bytes" => Some(BaseKind::Bytes),
        "timestamp" => Some(BaseKind::Timestamp),
        "record" => Some(BaseKind::Record),
        "list" => Some(BaseKind::List),
        "option" => Some(BaseKind::Option),
        "void" => Some(BaseKind::Void),
        _ => None,
    }
}

/// Strip a parametric type wrapper and return the inner type string.
///
/// `strip_parametric_wrapper("list<User>", "list<")` → `Some("User")`
/// `strip_parametric_wrapper("User", "list<")` → `None`
fn strip_parametric_wrapper<'a>(type_ref: &'a str, prefix: &str) -> Option<&'a str> {
    let lowered = type_ref.to_ascii_lowercase();
    if lowered.starts_with(prefix) && type_ref.ends_with('>') {
        let inner = &type_ref[prefix.len()..type_ref.len() - 1];
        if !inner.is_empty() {
            return Some(inner);
        }
    }
    None
}

/// Scan the graph for a type-defining node (`Define`, `Describe`, or `Error`)
/// whose `metadata.name` matches `name`.
///
/// Uses flat lookup — no scoping. Phase 4 will add scoped resolution.
pub(crate) fn find_type_node_by_name(graph: &dyn GraphBackend, name: &str) -> Option<NodeId> {
    graph.all_nodes_vec().into_iter().find_map(|node| {
        let is_type_node = matches!(
            node.pattern,
            Pattern::Define | Pattern::Describe | Pattern::Error
        );
        if is_type_node && node.metadata.name.as_deref() == Some(name) {
            Some(node.id)
        } else {
            None
        }
    })
}
