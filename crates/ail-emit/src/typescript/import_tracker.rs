use std::collections::BTreeMap;

/// Whether a referenced user-defined type was emitted from a `define` node.
///
/// `define` types expose both a branded type and a factory function (`create*`).
/// `describe` and `error` types only expose the type itself at usage sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeKind {
    Define,
    Describe,
    Error,
}

/// Tracks imports needed by a single emitted TypeScript file.
///
/// Each user-defined type reference registers the source file and the symbols
/// to import. `define` types get `{ T, createT }` because factory functions
/// appear in `createDescribe` parameter types. `describe` and `error` types
/// get `{ T }` only.
///
/// Uses `BTreeMap` for deterministic output order.
#[derive(Debug, Default)]
pub(crate) struct ImportTracker {
    /// Map from module path (e.g. `"./wallet_balance"`) to sorted symbol list.
    imports: BTreeMap<String, Vec<String>>,
    /// Set when contract mode `On` is active and at least one contract is emitted.
    /// Causes `render()` to prepend `import { pre, post, keep } from '../ail-runtime';`.
    pub(crate) needs_runtime: bool,
}

impl ImportTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Register a user-defined type `name` sourced from `module_path` with the
    /// given `kind`. The kind determines which symbols are imported.
    pub(crate) fn register(&mut self, name: &str, module_path: &str, kind: TypeKind) {
        let symbols = self.imports.entry(module_path.to_owned()).or_default();

        let type_sym = name.to_owned();
        if !symbols.contains(&type_sym) {
            symbols.push(type_sym);
        }

        if kind == TypeKind::Define {
            let factory_sym = format!("create{name}");
            if !symbols.contains(&factory_sym) {
                symbols.push(factory_sym);
            }
        }
    }

    /// Render all accumulated imports as TypeScript import statements.
    ///
    /// When `needs_runtime` is set, prepends
    /// `import { pre, post, keep } from '../ail-runtime';` (semantic order per spec)
    /// before any user-type imports.
    ///
    /// User-type imports are emitted one per source module, sorted by module path.
    pub(crate) fn render(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.needs_runtime {
            // Inline runtime lives at the output root; fn/ and repos/ files are one
            // level deep, so the relative import is always '../ail-runtime'.
            parts.push("import { pre, post, keep } from '../ail-runtime';".to_owned());
        }

        let type_imports = self
            .imports
            .iter()
            .map(|(module, symbols)| {
                let mut sorted = symbols.clone();
                sorted.sort();
                format!("import {{ {} }} from '{}';", sorted.join(", "), module)
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !type_imports.is_empty() {
            parts.push(type_imports);
        }

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker_renders_empty() {
        let tracker = ImportTracker::new();
        assert_eq!(tracker.render(), "");
    }

    #[test]
    fn define_type_imports_type_and_factory() {
        let mut tracker = ImportTracker::new();
        tracker.register("WalletBalance", "./wallet_balance", TypeKind::Define);
        let rendered = tracker.render();
        assert!(rendered.contains("WalletBalance"));
        assert!(rendered.contains("createWalletBalance"));
        assert!(rendered.contains("from './wallet_balance'"));
    }

    #[test]
    fn describe_type_imports_type_only() {
        let mut tracker = ImportTracker::new();
        tracker.register("User", "./user", TypeKind::Describe);
        let rendered = tracker.render();
        assert!(rendered.contains("User"));
        assert!(!rendered.contains("createUser"));
    }

    #[test]
    fn error_type_imports_type_only() {
        let mut tracker = ImportTracker::new();
        tracker.register("MyError", "./my_error", TypeKind::Error);
        let rendered = tracker.render();
        assert!(rendered.contains("MyError"));
        assert!(!rendered.contains("createMyError"));
    }

    #[test]
    fn multiple_imports_sorted_by_module() {
        let mut tracker = ImportTracker::new();
        tracker.register("Zebra", "./zebra", TypeKind::Describe);
        tracker.register("Alpha", "./alpha", TypeKind::Describe);
        let rendered = tracker.render();
        let alpha_pos = rendered.find("./alpha").unwrap();
        let zebra_pos = rendered.find("./zebra").unwrap();
        assert!(alpha_pos < zebra_pos);
    }

    #[test]
    fn duplicate_registration_ignored() {
        let mut tracker = ImportTracker::new();
        tracker.register("WalletBalance", "./wallet_balance", TypeKind::Define);
        tracker.register("WalletBalance", "./wallet_balance", TypeKind::Define);
        let rendered = tracker.render();
        // Should appear exactly once.
        assert_eq!(rendered.matches("WalletBalance,").count(), 1);
    }
}
