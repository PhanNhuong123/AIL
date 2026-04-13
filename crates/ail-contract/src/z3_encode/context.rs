use std::collections::HashMap;

use z3::ast::{Bool, Dynamic, Int, Real};

use crate::errors::EncodeError;

/// Z3 encoding context: maps variable path strings to Z3 AST nodes.
///
/// Callers pre-register each variable with the correct Z3 sort before invoking
/// [`encode_constraint`]. The key for a dotted-path ref `["sender", "balance"]` is
/// `"sender.balance"`. All nesting levels must be registered explicitly — the encoder
/// will return [`EncodeError::UnboundVariable`] for any path not in this context.
///
/// **Caller responsibility (Task 3.3)**: the verifier must walk each `TypedGraph` node's
/// parameter and result metadata and call `add_*_var` for every concrete field path
/// that appears in a contract expression, including nested record fields.
///
/// # Old-state variables
///
/// Variables used inside `old()` expressions are stored separately under an
/// `"old__{path}"` key. Register them with `add_old_*_var` before encoding
/// postconditions.
///
/// [`encode_constraint`]: super::encoder::encode_constraint
pub struct EncodeContext<'ctx> {
    /// The Z3 context used for all AST node construction.
    pub z3: &'ctx z3::Context,
    vars: HashMap<String, Dynamic<'ctx>>,
    old_vars: HashMap<String, Dynamic<'ctx>>,
}

impl<'ctx> EncodeContext<'ctx> {
    /// Create an empty encoding context backed by `z3`.
    pub fn new(z3: &'ctx z3::Context) -> Self {
        Self {
            z3,
            vars: HashMap::new(),
            old_vars: HashMap::new(),
        }
    }

    // ── Current-state registration ───────────────────────────────────────────

    /// Register an integer variable under `name` (e.g. `"sender.balance"`).
    ///
    /// Returns the Z3 `Int` constant so the caller can assert additional
    /// constraints on it directly.
    pub fn add_int_var(&mut self, name: &str) -> Int<'ctx> {
        let v = Int::new_const(self.z3, name);
        self.vars.insert(name.to_owned(), Dynamic::from_ast(&v));
        v
    }

    /// Register a real-number variable under `name`.
    pub fn add_real_var(&mut self, name: &str) -> Real<'ctx> {
        let v = Real::new_const(self.z3, name);
        self.vars.insert(name.to_owned(), Dynamic::from_ast(&v));
        v
    }

    /// Register a boolean variable under `name`.
    pub fn add_bool_var(&mut self, name: &str) -> Bool<'ctx> {
        let v = Bool::new_const(self.z3, name);
        self.vars.insert(name.to_owned(), Dynamic::from_ast(&v));
        v
    }

    // ── Old-state registration ───────────────────────────────────────────────

    /// Register an integer variable representing the pre-state snapshot of `name`.
    ///
    /// Internally stored under the key `"old__{name}"`. The Z3 constant is named
    /// `"old__{name}"` in the solver model.
    pub fn add_old_int_var(&mut self, name: &str) -> Int<'ctx> {
        let key = old_key(name);
        let v = Int::new_const(self.z3, key.as_str());
        self.old_vars.insert(key, Dynamic::from_ast(&v));
        v
    }

    /// Register a real-number variable representing the pre-state snapshot of `name`.
    pub fn add_old_real_var(&mut self, name: &str) -> Real<'ctx> {
        let key = old_key(name);
        let v = Real::new_const(self.z3, key.as_str());
        self.old_vars.insert(key, Dynamic::from_ast(&v));
        v
    }

    /// Register a boolean variable representing the pre-state snapshot of `name`.
    pub fn add_old_bool_var(&mut self, name: &str) -> Bool<'ctx> {
        let key = old_key(name);
        let v = Bool::new_const(self.z3, key.as_str());
        self.old_vars.insert(key, Dynamic::from_ast(&v));
        v
    }

    // ── Look-ups ─────────────────────────────────────────────────────────────

    /// Look up a current-state variable by its dotted-path segments.
    ///
    /// Returns `None` if the path was not registered.
    pub fn get_var(&self, path: &[String]) -> Option<&Dynamic<'ctx>> {
        self.vars.get(&var_key(path))
    }

    /// Look up a pre-state (`old()`) variable by its dotted-path segments.
    ///
    /// Returns `None` if the path was not registered as an old-var.
    pub fn get_old_var(&self, path: &[String]) -> Option<&Dynamic<'ctx>> {
        self.old_vars.get(&old_key(&var_key(path)))
    }

    /// Look up and return a current-state variable, or an [`EncodeError::UnboundVariable`].
    pub(super) fn require_var(&self, path: &[String]) -> Result<&Dynamic<'ctx>, EncodeError> {
        self.get_var(path)
            .ok_or_else(|| EncodeError::UnboundVariable {
                name: var_key(path),
            })
    }

    /// Look up and return a pre-state variable, or an [`EncodeError::UnboundVariable`].
    pub(super) fn require_old_var(&self, path: &[String]) -> Result<&Dynamic<'ctx>, EncodeError> {
        self.get_old_var(path)
            .ok_or_else(|| EncodeError::UnboundVariable {
                name: format!("old({})", var_key(path)),
            })
    }

    /// Evaluate every registered variable in `model` and return `(name, value)` pairs.
    ///
    /// Both current-state (`vars`) and old-state (`old_vars`) entries are included.
    /// The returned list is sorted by name for deterministic output in counterexamples.
    /// Variables that the model does not interpret (e.g. unconstrained universals) are
    /// omitted from the result.
    pub fn format_model(&self, model: &z3::Model<'ctx>) -> Vec<(String, String)> {
        let mut assignments: Vec<(String, String)> = Vec::new();

        for (name, dyn_var) in &self.vars {
            if let Some(interp) = model.eval(dyn_var, true) {
                assignments.push((name.clone(), interp.to_string()));
            }
        }
        for (key, dyn_var) in &self.old_vars {
            if let Some(interp) = model.eval(dyn_var, true) {
                assignments.push((key.clone(), interp.to_string()));
            }
        }

        assignments.sort_by(|a, b| a.0.cmp(&b.0));
        assignments
    }
}

/// Join path segments into a dotted key: `["sender", "balance"]` → `"sender.balance"`.
pub(super) fn var_key(path: &[String]) -> String {
    path.join(".")
}

/// Build the storage key for an old-state variable: `"sender.balance"` → `"old__sender.balance"`.
fn old_key(name: &str) -> String {
    format!("old__{name}")
}
