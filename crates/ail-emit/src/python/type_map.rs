use crate::constants::TYPE_MAP;
use crate::types::ImportSet;

/// Python builtins and common stdlib exception names that are never user-defined.
///
/// These names pass through `resolve_python_type` as bare names but must not be added
/// to `cross_file_types` — they are always available without an import.
const PYTHON_BUILTINS: &[&str] = &[
    // Numeric and scalar builtins
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "None",
    // stdlib types
    "datetime",
    "object",
    "list",
    "dict",
    "tuple",
    "set",
    "frozenset",
    // Common exception classes
    "Exception",
    "BaseException",
    "AssertionError",
    "ValueError",
    "TypeError",
    "RuntimeError",
    "KeyError",
    "IndexError",
    "AttributeError",
    "NotImplementedError",
    "NameError",
];

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

/// Register a type reference as needing a cross-file import from `generated/types.py`.
///
/// Recursively unwraps `list<T>` and `option<T>` containers. Skips:
/// - AIL primitive type names (those in `TYPE_MAP`)
/// - Common Python builtins and stdlib exception names
///
/// Everything else is treated as a user-defined type and inserted into
/// `imports.cross_file_types` so that `ImportSet::render()` can emit
/// `from .types import T1, T2`.
///
/// **Assumption (v0.1):** All user-defined types (records and exceptions) live in
/// `generated/types.py`. If a future task splits types across multiple files, this
/// function must be updated to route by type kind.
pub(crate) fn register_cross_file_type(type_ref: &str, imports: &mut ImportSet) {
    let trimmed = type_ref.trim();

    // Unwrap list<T> — recurse on the inner type.
    if let Some(inner) = strip_wrapper(trimmed, "list") {
        register_cross_file_type(inner, imports);
        return;
    }
    // Unwrap option<T> — recurse on the inner type.
    if let Some(inner) = strip_wrapper(trimmed, "option") {
        register_cross_file_type(inner, imports);
        return;
    }

    // Skip AIL primitive type names (they map to Python builtins via TYPE_MAP).
    for &(ail_name, _py) in TYPE_MAP {
        if trimmed == ail_name {
            return;
        }
    }

    // Skip Python builtins and common stdlib exceptions.
    if PYTHON_BUILTINS.contains(&trimmed) || trimmed.is_empty() {
        return;
    }

    // User-defined type: needs a cross-file import.
    imports.cross_file_types.insert(trimmed.to_owned());
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

    // ── register_cross_file_type ──────────────────────────────────────────────

    fn register(type_ref: &str) -> ImportSet {
        let mut imports = ImportSet::new();
        register_cross_file_type(type_ref, &mut imports);
        imports
    }

    #[test]
    fn register_cross_file_primitive_not_added() {
        let imports = register("number");
        assert!(imports.cross_file_types.is_empty());

        let imports = register("text");
        assert!(imports.cross_file_types.is_empty());

        let imports = register("boolean");
        assert!(imports.cross_file_types.is_empty());

        let imports = register("integer");
        assert!(imports.cross_file_types.is_empty());
    }

    #[test]
    fn register_cross_file_user_type_added() {
        let imports = register("WalletBalance");
        assert!(imports.cross_file_types.contains("WalletBalance"));
    }

    #[test]
    fn register_cross_file_list_unwrapped() {
        let imports = register("list<WalletBalance>");
        assert!(imports.cross_file_types.contains("WalletBalance"));
    }

    #[test]
    fn register_cross_file_option_unwrapped() {
        let imports = register("option<Balance>");
        assert!(imports.cross_file_types.contains("Balance"));
    }

    #[test]
    fn register_cross_file_builtin_not_added() {
        let imports = register("Exception");
        assert!(imports.cross_file_types.is_empty());

        let imports = register("AssertionError");
        assert!(imports.cross_file_types.is_empty());
    }

    #[test]
    fn register_cross_file_python_builtin_not_added() {
        let imports = register("int");
        assert!(imports.cross_file_types.is_empty());

        let imports = register("str");
        assert!(imports.cross_file_types.is_empty());
    }

    #[test]
    fn register_cross_file_nested_list_option_unwrapped() {
        // list<option<MyType>> → inner option → inner MyType
        let imports = register("list<option<Transfer>>");
        assert!(imports.cross_file_types.contains("Transfer"));
    }
}
