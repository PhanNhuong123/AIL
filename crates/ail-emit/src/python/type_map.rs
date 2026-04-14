use crate::constants::TYPE_MAP;
use crate::types::ImportSet;

/// Resolve an AIL type reference to a Python type annotation.
///
/// - Primitive types (number, text, etc.) map to Python builtins.
/// - `list<T>` maps to `list[T]` (recursive).
/// - `option<T>` maps to `T | None` (recursive).
/// - `timestamp` maps to `datetime` (sets `needs_datetime`).
/// - User-defined types pass through as bare names (PEP 563 deferred).
pub(crate) fn resolve_python_type(type_ref: &str, imports: &mut ImportSet) -> String {
    let trimmed = type_ref.trim();

    // Check for parameterized types: list<T> or option<T>.
    if let Some(inner) = strip_wrapper(trimmed, "list") {
        let inner_py = resolve_python_type(inner, imports);
        return format!("list[{inner_py}]");
    }
    if let Some(inner) = strip_wrapper(trimmed, "option") {
        let inner_py = resolve_python_type(inner, imports);
        return format!("{inner_py} | None");
    }

    // Check primitive type map.
    for &(ail, py) in TYPE_MAP {
        if trimmed == ail {
            if py == "datetime" {
                imports.needs_datetime = true;
            }
            return py.to_owned();
        }
    }

    // User-defined type — bare name (deferred by PEP 563).
    trimmed.to_owned()
}

/// Try to strip a wrapper like `list<...>` or `option<...>`, returning the inner type.
fn strip_wrapper<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let s = s.strip_prefix(prefix)?;
    let s = s.strip_prefix('<')?;
    let s = s.strip_suffix('>')?;
    Some(s.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(s: &str) -> (String, ImportSet) {
        let mut imports = ImportSet::new();
        let result = resolve_python_type(s, &mut imports);
        (result, imports)
    }

    #[test]
    fn emit_map_number_to_float() {
        let (r, _) = resolve("number");
        assert_eq!(r, "float");
    }

    #[test]
    fn emit_map_integer_to_int() {
        let (r, _) = resolve("integer");
        assert_eq!(r, "int");
    }

    #[test]
    fn emit_map_text_to_str() {
        let (r, _) = resolve("text");
        assert_eq!(r, "str");
    }

    #[test]
    fn emit_map_boolean_to_bool() {
        let (r, _) = resolve("boolean");
        assert_eq!(r, "bool");
    }

    #[test]
    fn emit_map_list_of_number() {
        let (r, _) = resolve("list<number>");
        assert_eq!(r, "list[float]");
    }

    #[test]
    fn emit_map_option_of_text() {
        let (r, _) = resolve("option<text>");
        assert_eq!(r, "str | None");
    }

    #[test]
    fn emit_map_user_defined_type() {
        let (r, _) = resolve("WalletBalance");
        assert_eq!(r, "WalletBalance");
    }

    #[test]
    fn emit_map_nested_list_option() {
        let (r, _) = resolve("list<option<text>>");
        assert_eq!(r, "list[str | None]");
    }

    #[test]
    fn emit_map_timestamp_sets_import() {
        let (r, imports) = resolve("timestamp");
        assert_eq!(r, "datetime");
        assert!(imports.needs_datetime);
    }

    #[test]
    fn emit_map_bytes() {
        let (r, _) = resolve("bytes");
        assert_eq!(r, "bytes");
    }
}
