use ail_graph::AilGraph;
use ail_types::BuiltinSemanticType;

use ail_graph::types::Pattern;

/// The Z3 sort to which an AIL type maps.
///
/// Only scalar sorts (Int, Real, Bool) have direct Z3 representations.
/// Record types (`Describe` nodes) and unrecognised types are `Uninterpreted`:
/// their variables are skipped in the encode context and their type constraints
/// are not asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Z3Sort {
    Int,
    Real,
    Bool,
    /// Record type, text type, or truly unknown — cannot be encoded as a Z3 scalar.
    Uninterpreted,
}

/// Map an AIL type-ref string to the appropriate Z3 sort.
///
/// **Lookup order:**
/// 1. Base primitives (`"integer"`, `"number"`, `"text"`, `"bool"`, `"boolean"`).
/// 2. Built-in semantic types via [`BuiltinSemanticType::from_name`].
/// 3. User-defined graph types: `Define` nodes recurse to their base type;
///    `Describe` nodes are always `Uninterpreted`.
/// 4. Fallback: `Uninterpreted`.
///
/// **Important:** Unknown custom types always return `Uninterpreted`, not `Int`.
/// Encoding a record type as an integer would produce nonsensical Z3 assertions.
pub(super) fn sort_for_type_ref(type_ref: &str, graph: &AilGraph) -> Z3Sort {
    sort_for_type_ref_inner(type_ref, graph, 0)
}

/// Recursion-guarded inner implementation. The depth limit prevents infinite
/// loops on pathological `Define` chains (in practice AIL graphs have no cycles
/// because `ValidGraph` enforces acyclicity, but we keep the guard for safety).
fn sort_for_type_ref_inner(type_ref: &str, graph: &AilGraph, depth: u8) -> Z3Sort {
    const MAX_DEPTH: u8 = 16;
    if depth > MAX_DEPTH {
        return Z3Sort::Uninterpreted;
    }

    // ── 1. Base primitive names ───────────────────────────────────────────────
    match type_ref {
        "integer" | "Integer" => return Z3Sort::Int,
        "number" | "Number" => return Z3Sort::Real,
        "text" | "Text" | "string" | "String" => return Z3Sort::Uninterpreted,
        "bool" | "Bool" | "boolean" | "Boolean" => return Z3Sort::Bool,
        _ => {}
    }

    // ── 2. Built-in semantic types ────────────────────────────────────────────
    if let Some(builtin) = BuiltinSemanticType::from_name(type_ref) {
        return match builtin {
            BuiltinSemanticType::PositiveInteger | BuiltinSemanticType::NonNegativeInteger => {
                Z3Sort::Int
            }
            BuiltinSemanticType::PositiveAmount | BuiltinSemanticType::Percentage => Z3Sort::Real,
            BuiltinSemanticType::NonEmptyText
            | BuiltinSemanticType::EmailAddress
            | BuiltinSemanticType::Identifier => Z3Sort::Uninterpreted,
        };
    }

    // ── 3. User-defined graph types ───────────────────────────────────────────
    for node in graph.all_nodes() {
        let name_matches = node
            .metadata
            .name
            .as_deref()
            .map(|n| n == type_ref)
            .unwrap_or(false);

        if !name_matches {
            continue;
        }

        return match node.pattern {
            // Define is a type alias: resolve to its base type.
            Pattern::Define => {
                if let Some(base) = &node.metadata.base_type {
                    sort_for_type_ref_inner(base, graph, depth + 1)
                } else {
                    Z3Sort::Uninterpreted
                }
            }
            // Describe is a record: cannot be represented as a Z3 scalar.
            Pattern::Describe => Z3Sort::Uninterpreted,
            // Any other pattern carrying a matching name is treated as uninterpreted.
            _ => Z3Sort::Uninterpreted,
        };
    }

    // ── 4. Fallback ───────────────────────────────────────────────────────────
    Z3Sort::Uninterpreted
}
