use ail_graph::{GraphBackend, Node, Pattern};

/// Convert a space- or underscore-separated intent string to a camelCase function name.
///
/// `"transfer money safely"` → `"transferMoneySafely"`
/// `"transfer_money_safely"` → `"transferMoneySafely"`
pub(crate) fn to_camel_case_fn(intent: &str) -> String {
    let words: Vec<&str> = intent
        .trim()
        .split([' ', '_'])
        .filter(|s| !s.is_empty())
        .collect();

    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 {
                w.to_lowercase()
            } else {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        let upper: String = first.to_uppercase().collect();
                        upper + &chars.as_str().to_lowercase()
                    }
                }
            }
        })
        .collect()
}

/// Convert a snake_case variable name to camelCase for TypeScript output.
///
/// `"new_balance"` → `"newBalance"`, `"sender_id"` → `"senderId"`.
///
/// Complex expressions containing `.`, spaces, or operators are returned unchanged
/// because they are field-access chains or compound expressions, not identifiers.
///
/// Applied to:
/// - `let` binding names
/// - Value parts of `(key, value)` pairs in fetch/update/remove/return/raise
///
/// Field name *keys* are intentionally left snake_case.
pub(crate) fn to_camel_case_var(name: &str) -> String {
    let trimmed = name.trim();
    // Leave complex expressions as-is (field access, arithmetic, strings, etc.)
    if trimmed.chars().any(|c| {
        matches!(
            c,
            '.' | ' ' | '+' | '-' | '*' | '/' | '(' | ')' | '[' | ']' | ',' | '"' | '\''
        )
    }) {
        return trimmed.to_owned();
    }
    // Only convert if the identifier contains underscores (i.e., it is snake_case).
    // Already-camelCase values (like `userId`, `newBalance`) are returned unchanged
    // to avoid lowercasing them.
    if !trimmed.contains('_') {
        return trimmed.to_owned();
    }
    to_camel_case_fn(trimmed)
}

/// Detect whether a `do` node's function body requires `async`.
///
/// Returns `true` if any descendant node (at any depth) has pattern
/// `Fetch`, `Save`, `Update`, `Remove`, `Together`, or `Retry`.
/// - `Together` always emits `await source.transaction(...)`.
/// - `Retry` always emits `await new Promise(resolve => setTimeout(...))`.
pub(crate) fn detect_is_async(graph: &dyn GraphBackend, node: &Node) -> bool {
    detect_async_recursive(graph, node)
}

fn detect_async_recursive(graph: &dyn GraphBackend, node: &Node) -> bool {
    let children = match node.children.as_deref() {
        Some(c) => c,
        None => return false,
    };
    for &child_id in children {
        let child_owned = match graph.get_node(child_id).ok().flatten() {
            Some(n) => n,
            None => continue,
        };
        match child_owned.pattern {
            Pattern::Fetch
            | Pattern::Save
            | Pattern::Update
            | Pattern::Remove
            | Pattern::Together
            | Pattern::Retry => return true,
            _ => {}
        }
        if detect_async_recursive(graph, &child_owned) {
            return true;
        }
    }
    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_fn_space_separated() {
        assert_eq!(
            to_camel_case_fn("transfer money safely"),
            "transferMoneySafely"
        );
    }

    #[test]
    fn camel_fn_underscore_separated() {
        assert_eq!(
            to_camel_case_fn("transfer_money_safely"),
            "transferMoneySafely"
        );
    }

    #[test]
    fn camel_fn_single_word() {
        assert_eq!(to_camel_case_fn("compute"), "compute");
    }

    #[test]
    fn camel_fn_two_words() {
        assert_eq!(to_camel_case_fn("get user"), "getUser");
    }

    #[test]
    fn camel_var_simple() {
        assert_eq!(to_camel_case_var("new_balance"), "newBalance");
    }

    #[test]
    fn camel_var_sender_id() {
        assert_eq!(to_camel_case_var("sender_id"), "senderId");
    }

    #[test]
    fn camel_var_field_access_unchanged() {
        assert_eq!(to_camel_case_var("sender.balance"), "sender.balance");
    }

    #[test]
    fn camel_var_complex_expr_unchanged() {
        assert_eq!(
            to_camel_case_var("sender.balance - amount"),
            "sender.balance - amount"
        );
    }

    #[test]
    fn camel_var_simple_word_unchanged() {
        assert_eq!(to_camel_case_var("amount"), "amount");
    }

    #[test]
    fn camel_var_three_words() {
        assert_eq!(to_camel_case_var("new_sender_balance"), "newSenderBalance");
    }
}
