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
    /// Emits one `import { ... } from '...'` line per source module, sorted by
    /// module path for deterministic output.
    pub(crate) fn render(&self) -> String {
        if self.imports.is_empty() {
            return String::new();
        }

        self.imports
            .iter()
            .map(|(module, symbols)| {
                let mut sorted = symbols.clone();
                sorted.sort();
                format!("import {{ {} }} from '{}';", sorted.join(", "), module)
            })
            .collect::<Vec<_>>()
            .join("\n")
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
