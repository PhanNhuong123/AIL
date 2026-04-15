use std::collections::BTreeSet;

use crate::types::FileOwnership;

/// The result of emitting Python code from a VerifiedGraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitOutput {
    pub files: Vec<EmittedFile>,
}

/// A single emitted file with its relative path, content, and ownership policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedFile {
    /// Relative path from the output root (e.g. "generated/types.py").
    pub path: String,
    /// Full file content including imports.
    pub content: String,
    /// Whether the emitter always overwrites this file or creates it only once.
    pub ownership: FileOwnership,
}

/// Tracks which Python imports are needed across a generated file.
///
/// Accumulated during emission, then rendered into an import preamble.
#[derive(Debug, Default)]
pub(crate) struct ImportSet {
    pub needs_dataclass: bool,
    pub needs_re: bool,
    pub needs_keep: bool,
    pub needs_pre: bool,
    pub needs_post: bool,
    pub needs_datetime: bool,
    /// `import asyncio` — needed by Together blocks and async Retry.
    pub needs_asyncio: bool,
    /// `import time` — needed by sync Retry.
    pub needs_time: bool,
    /// Adds `transaction` to the `from ail_runtime import ...` line.
    pub needs_transaction: bool,
    /// User-defined type names that need `from .types import ...` in the preamble.
    ///
    /// Populated during function body emission when types are used as runtime
    /// values (constructor calls, exception constructors, repository type args).
    /// Uses `BTreeSet` for deterministic sorted output.
    pub cross_file_types: BTreeSet<String>,
}

impl ImportSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the import preamble. Always starts with `from __future__ import annotations`.
    pub fn render(&self) -> String {
        let mut lines = vec!["from __future__ import annotations".to_owned()];

        if self.needs_dataclass {
            lines.push("from dataclasses import dataclass".to_owned());
        }
        if self.needs_datetime {
            lines.push("from datetime import datetime".to_owned());
        }
        if self.needs_re {
            lines.push("import re".to_owned());
        }
        if self.needs_asyncio {
            lines.push("import asyncio".to_owned());
        }
        if self.needs_time {
            lines.push("import time".to_owned());
        }

        // Collect ail_runtime imports.
        let mut runtime_imports = Vec::new();
        if self.needs_keep {
            runtime_imports.push("keep");
        }
        if self.needs_pre {
            runtime_imports.push("pre");
        }
        if self.needs_post {
            runtime_imports.push("post");
        }
        if self.needs_transaction {
            runtime_imports.push("transaction");
        }
        if !runtime_imports.is_empty() {
            lines.push(format!(
                "from ail_runtime import {}",
                runtime_imports.join(", ")
            ));
        }

        // Cross-file imports: user-defined types from generated/types.py.
        if !self.cross_file_types.is_empty() {
            let names: Vec<&str> = self.cross_file_types.iter().map(String::as_str).collect();
            lines.push(format!("from .types import {}", names.join(", ")));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_set_always_includes_future_annotations() {
        let imports = ImportSet::new();
        let rendered = imports.render();
        assert_eq!(rendered, "from __future__ import annotations");
    }

    #[test]
    fn import_set_renders_all_needed_imports() {
        let imports = ImportSet {
            needs_dataclass: true,
            needs_re: true,
            needs_keep: true,
            needs_pre: false,
            needs_post: false,
            needs_datetime: true,
            ..Default::default()
        };
        let rendered = imports.render();
        assert!(rendered.contains("from __future__ import annotations"));
        assert!(rendered.contains("from dataclasses import dataclass"));
        assert!(rendered.contains("from datetime import datetime"));
        assert!(rendered.contains("import re"));
        assert!(rendered.contains("from ail_runtime import keep"));
    }

    #[test]
    fn import_set_combines_runtime_imports() {
        let imports = ImportSet {
            needs_keep: true,
            needs_pre: true,
            needs_post: true,
            ..Default::default()
        };
        let rendered = imports.render();
        assert!(rendered.contains("from ail_runtime import keep, pre, post"));
    }

    #[test]
    fn cross_file_types_empty_no_import_line() {
        let imports = ImportSet::new();
        let rendered = imports.render();
        assert!(!rendered.contains("from .types import"));
    }

    #[test]
    fn cross_file_types_single_renders_import() {
        let mut imports = ImportSet::new();
        imports.cross_file_types.insert("MyType".to_owned());
        let rendered = imports.render();
        assert!(rendered.contains("from .types import MyType"));
    }

    #[test]
    fn cross_file_types_sorted_alphabetically() {
        let mut imports = ImportSet::new();
        imports.cross_file_types.insert("Zebra".to_owned());
        imports.cross_file_types.insert("Alpha".to_owned());
        imports.cross_file_types.insert("Middle".to_owned());
        let rendered = imports.render();
        // BTreeSet guarantees alphabetical order.
        assert!(rendered.contains("from .types import Alpha, Middle, Zebra"));
    }

    #[test]
    fn cross_file_types_rendered_after_runtime_imports() {
        let mut imports = ImportSet {
            needs_pre: true,
            ..Default::default()
        };
        imports.cross_file_types.insert("MyType".to_owned());
        let rendered = imports.render();
        let runtime_pos = rendered.find("from ail_runtime import").unwrap();
        let types_pos = rendered.find("from .types import").unwrap();
        assert!(
            types_pos > runtime_pos,
            "cross-file import must come after ail_runtime import"
        );
    }
}
