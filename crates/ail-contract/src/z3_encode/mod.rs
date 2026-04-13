//! Z3 encoding for AIL contract expressions (Phase 3 Task 3.2).
//!
//! Converts [`ConstraintExpr`] AST nodes from `ail-types` into Z3 Bool expressions
//! that can be asserted in a Z3 solver. The encoding layer is a prerequisite for
//! task 3.3 (Z3 Verification), which drives satisfiability checks and counterexample
//! extraction.
//!
//! # Entry points
//!
//! - [`EncodeContext`] — pre-register all variable paths before encoding.
//! - [`encode_constraint`] — encode a `ConstraintExpr` into a Z3 `Bool`.
//! - [`encode_value_int`] / [`encode_value_real`] / [`encode_value_bool`] — encode
//!   a `ValueExpr` into the requested Z3 sort.
//! - [`encode_type_constraint`] — encode a `BuiltinSemanticType` bound as Z3 assertions.
//!
//! [`ConstraintExpr`]: ail_types::ConstraintExpr

pub mod context;
pub mod encoder;
pub mod type_constraints;

#[cfg(test)]
mod tests;

pub use context::EncodeContext;
pub use encoder::{encode_constraint, encode_value_bool, encode_value_int, encode_value_real};
pub use type_constraints::encode_type_constraint;
