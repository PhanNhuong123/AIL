/// Mapping from AIL primitive base type names to TypeScript type annotations.
///
/// `integer` maps to `number`; the define emitter adds an implicit `Number.isInteger` check.
/// `record` maps to `Record<string, unknown>` (TypeScript structural object type).
/// `timestamp` maps to `Date`.
pub(crate) const TS_TYPE_MAP: &[(&str, &str)] = &[
    ("number", "number"),
    ("integer", "number"),
    ("text", "string"),
    ("boolean", "boolean"),
    ("bytes", "Uint8Array"),
    ("timestamp", "Date"),
    ("record", "Record<string, unknown>"),
    ("void", "void"),
];

/// Resolve an AIL type reference to a TypeScript type annotation string.
///
/// - Primitive types map via `TS_TYPE_MAP`.
/// - `list<T>` → `readonly T[]`.
/// - `option<T>` → `T | null`.
/// - User-defined types pass through as bare names (TypeScript handles circular
///   interfaces structurally; no `import type` needed in v2.0).
pub(crate) fn resolve_ts_type(type_ref: &str) -> String {
    let trimmed = type_ref.trim();

    if let Some(inner) = strip_wrapper(trimmed, "list") {
        let inner_ts = resolve_ts_type(inner);
        return format!("readonly {inner_ts}[]");
    }
    if let Some(inner) = strip_wrapper(trimmed, "option") {
        let inner_ts = resolve_ts_type(inner);
        return format!("{inner_ts} | null");
    }

    for &(ail, ts) in TS_TYPE_MAP {
        if trimmed == ail {
            return ts.to_owned();
        }
    }

    // User-defined type — bare name.
    trimmed.to_owned()
}

/// Return true if `type_ref` is an AIL primitive (not a user-defined type).
pub(crate) fn is_primitive_type(type_ref: &str) -> bool {
    let trimmed = type_ref.trim();
    // Parameterized wrappers are not user-defined.
    if strip_wrapper(trimmed, "list").is_some() || strip_wrapper(trimmed, "option").is_some() {
        return true;
    }
    TS_TYPE_MAP.iter().any(|&(ail, _)| ail == trimmed)
}

/// Convert a PascalCase or camelCase type name to snake_case for file naming.
///
/// Handles consecutive capitals: `UserID` → `user_id`, `WalletBalance` → `wallet_balance`.
pub(crate) fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);

    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && chars[i - 1].is_ascii_lowercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            let prev_upper = i > 0 && chars[i - 1].is_ascii_uppercase();

            if i > 0 && (prev_lower || (next_lower && prev_upper)) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }

    out
}

/// Try to strip a wrapper like `list<...>` or `option<...>`, returning the inner type string.
pub(crate) fn strip_wrapper<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let s = s.strip_prefix(prefix)?;
    let s = s.strip_prefix('<')?;
    let s = s.strip_suffix('>')?;
    Some(s.trim())
}

/// Collect the innermost user-defined type names from a (possibly nested) type ref,
/// recursing through `list<T>` and `option<T>` wrappers.
pub(crate) fn collect_user_types(type_ref: &str) -> Vec<String> {
    let trimmed = type_ref.trim();

    if let Some(inner) = strip_wrapper(trimmed, "list") {
        return collect_user_types(inner);
    }
    if let Some(inner) = strip_wrapper(trimmed, "option") {
        return collect_user_types(inner);
    }

    if !is_primitive_type(trimmed) && !trimmed.is_empty() {
        vec![trimmed.to_owned()]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_map_number() {
        assert_eq!(resolve_ts_type("number"), "number");
    }

    #[test]
    fn ts_map_integer_to_number() {
        assert_eq!(resolve_ts_type("integer"), "number");
    }

    #[test]
    fn ts_map_text_to_string() {
        assert_eq!(resolve_ts_type("text"), "string");
    }

    #[test]
    fn ts_map_boolean() {
        assert_eq!(resolve_ts_type("boolean"), "boolean");
    }

    #[test]
    fn ts_map_bytes() {
        assert_eq!(resolve_ts_type("bytes"), "Uint8Array");
    }

    #[test]
    fn ts_map_timestamp_to_date() {
        assert_eq!(resolve_ts_type("timestamp"), "Date");
    }

    #[test]
    fn ts_map_void() {
        assert_eq!(resolve_ts_type("void"), "void");
    }

    #[test]
    fn ts_map_record() {
        assert_eq!(resolve_ts_type("record"), "Record<string, unknown>");
    }

    #[test]
    fn ts_map_list_of_number() {
        assert_eq!(resolve_ts_type("list<number>"), "readonly number[]");
    }

    #[test]
    fn ts_map_option_of_text() {
        assert_eq!(resolve_ts_type("option<text>"), "string | null");
    }

    #[test]
    fn ts_map_user_defined() {
        assert_eq!(resolve_ts_type("WalletBalance"), "WalletBalance");
    }

    #[test]
    fn ts_map_nested_list_option() {
        assert_eq!(
            resolve_ts_type("list<option<text>>"),
            "readonly string | null[]"
        );
    }

    #[test]
    fn to_snake_wallet_balance() {
        assert_eq!(to_snake_case("WalletBalance"), "wallet_balance");
    }

    #[test]
    fn to_snake_user_id_consecutive_caps() {
        assert_eq!(to_snake_case("UserID"), "user_id");
    }

    #[test]
    fn to_snake_single_word() {
        assert_eq!(to_snake_case("User"), "user");
    }

    #[test]
    fn to_snake_insufficient_balance_error() {
        assert_eq!(
            to_snake_case("InsufficientBalanceError"),
            "insufficient_balance_error"
        );
    }

    #[test]
    fn collect_user_types_primitive_empty() {
        assert!(collect_user_types("number").is_empty());
    }

    #[test]
    fn collect_user_types_user_defined() {
        assert_eq!(collect_user_types("WalletBalance"), vec!["WalletBalance"]);
    }

    #[test]
    fn collect_user_types_list_unwrapped() {
        assert_eq!(
            collect_user_types("list<WalletBalance>"),
            vec!["WalletBalance"]
        );
    }

    #[test]
    fn collect_user_types_option_unwrapped() {
        assert_eq!(collect_user_types("option<UserId>"), vec!["UserId"]);
    }
}
