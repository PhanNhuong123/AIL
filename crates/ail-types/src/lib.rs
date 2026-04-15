//! `ail-types` — constraint AST, expression evaluator, built-in semantic types, and `TypedGraph`.
//!
//! This crate is the second stage of the AIL compiler pipeline. It owns:
//! - Constraint and value expression ASTs ([`ConstraintExpr`], [`ValueExpr`]).
//! - Expression parsing and display roundtrip.
//! - A runtime-style evaluator over [`EvalContext`] and [`Value`].
//! - Built-in semantic type validators ([`BuiltinSemanticType`]).
//! - Type reference resolution and graph-wide type checking ([`type_check`]).
//! - `TypedGraph`, the second hard pipeline gate.
//!
//! ## Pipeline position
//!
//! ```text
//! ValidGraph + ContextPackets → type_check() → TypedGraph → (ail-contract)
//! ```
//!
//! ## Entry points
//!
//! - [`type_check`] — consume a `ValidGraph` and return `TypedGraph` or accumulated errors.
//! - [`eval_constraint`] / [`eval_value`] — evaluate constraint/value expressions at runtime.
//! - [`parse_constraint_expr`] — parse an expression string into [`ConstraintExpr`].

pub mod builtins;
mod checker;
mod errors;
pub mod eval;
mod expr;
mod types;

pub use builtins::BuiltinSemanticType;
pub use checker::type_check;
pub use errors::{EvalError, ParseError, TypeError};
pub use eval::{eval_constraint, eval_value, EvalContext, Value};
pub use expr::{parse_constraint_expr, parse_value_expr};
pub use types::{ArithOp, CompareOp, ConstraintExpr, LiteralValue, TypedGraph, ValueExpr};
