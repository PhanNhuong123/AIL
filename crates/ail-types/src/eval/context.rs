use std::collections::HashMap;

use crate::errors::EvalError;
use crate::eval::Value;

/// Evaluation context: current bindings plus optional old-state snapshot.
///
/// `old_bindings` is `Some` only inside a post-condition (`promise after`) context.
/// Evaluating `old(expr)` outside a post-condition returns `EvalError::OldOutsidePostCondition`.
#[derive(Debug, Clone)]
pub struct EvalContext {
    pub bindings: HashMap<String, Value>,
    pub old_bindings: Option<HashMap<String, Value>>,
}

impl EvalContext {
    /// Create a context with current bindings only (pre-condition or always context).
    pub fn new(bindings: HashMap<String, Value>) -> Self {
        Self { bindings, old_bindings: None }
    }

    /// Create a context with both current and old bindings (post-condition context).
    pub fn with_old(
        bindings: HashMap<String, Value>,
        old_bindings: HashMap<String, Value>,
    ) -> Self {
        Self { bindings, old_bindings: Some(old_bindings) }
    }

    /// Return a new context with an additional binding — used for quantifier variable scoping.
    ///
    /// `old_bindings` are inherited unchanged so that `old()` still works inside quantifier bodies.
    pub fn bind(&self, name: impl Into<String>, value: Value) -> Self {
        let mut new_bindings = self.bindings.clone();
        new_bindings.insert(name.into(), value);
        Self { bindings: new_bindings, old_bindings: self.old_bindings.clone() }
    }

    /// Return a context where `bindings` are the old snapshot values.
    ///
    /// Used by `eval_value` when evaluating the inner expression of `old(...)`.
    /// Returns `EvalError::OldOutsidePostCondition` if no old bindings are available.
    pub fn as_old_context(&self) -> Result<Self, EvalError> {
        match &self.old_bindings {
            Some(old) => Ok(Self { bindings: old.clone(), old_bindings: None }),
            None => Err(EvalError::OldOutsidePostCondition),
        }
    }
}
