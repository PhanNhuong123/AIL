/// The result of emitting Python code from a VerifiedGraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitOutput {
    pub files: Vec<EmittedFile>,
}

/// A single emitted file with its relative path and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedFile {
    /// Relative path from the output root (e.g. "generated/types.py").
    pub path: String,
    /// Full file content including imports.
    pub content: String,
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
        if !runtime_imports.is_empty() {
            lines.push(format!(
                "from ail_runtime import {}",
                runtime_imports.join(", ")
            ));
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
}
