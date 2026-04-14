/// Mapping from AIL primitive base type names to Python type annotations.
pub(crate) const TYPE_MAP: &[(&str, &str)] = &[
    ("number", "float"),
    ("integer", "int"),
    ("text", "str"),
    ("boolean", "bool"),
    ("bytes", "bytes"),
    ("timestamp", "datetime"),
    ("void", "None"),
];

pub(crate) const PYTHON_INDENT: &str = "    ";
