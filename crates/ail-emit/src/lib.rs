//! `ail-emit` — Multi-target code generator for the AIL compiler pipeline.
//!
//! This crate is the fifth stage of the AIL compiler pipeline. It owns:
//! - Python type definitions from `Define`, `Describe`, and `Error` patterns ([`emit_type_definitions`]).
//! - Python function bodies with injected contract assertions ([`emit_function_definitions`]).
//! - Scaffold files (pytest stubs, `__init__.py`) written once and developer-owned thereafter ([`emit_scaffold_files`]).
//! - Source-map JSON (`functions.ailmap.json`) for tracing emitted code back to `.ail` nodes.
//! - TypeScript type definitions (branded types, interfaces, Error subclasses) ([`emit_ts_type_definitions`]).
//! - TypeScript function definitions (async functions, repository interfaces) ([`emit_ts_function_definitions`]).
//!
//! ## Pipeline position
//!
//! ```text
//! VerifiedGraph → emit_type_definitions()            → types.py
//!              → emit_function_definitions()         → functions.py + test_contracts.py
//!              → emit_scaffold_files()               → scaffolded/__init__.py (write-once)
//!              → emit_ts_type_definitions()          → types/*.ts + errors/*.ts + barrel index.ts
//!              → emit_ts_function_definitions()      → fn/*.ts + repos/*.ts + barrel index.ts files
//! ```
//!
//! ## Entry points
//!
//! - [`emit_type_definitions`] — emit Python type stubs for the full graph.
//! - [`emit_function_definitions`] — emit Python function bodies with contract assertions.
//! - [`emit_scaffold_files`] — emit developer-owned scaffold files (never overwritten after first write).
//! - [`emit_ts_type_definitions`] — emit TypeScript type files (task 9.1).
//! - [`emit_ts_function_definitions`] — emit TypeScript function files (task 9.2).

mod constants;
mod errors;
mod python;
mod types;
mod typescript;

pub use errors::EmitError;
pub use python::emit_functions::emit_function_definitions;
pub use python::emit_types::emit_type_definitions;
pub use python::scaffold::emit_scaffold_files;
pub use types::{ContractMode, EmitConfig, EmitOutput, EmittedFile, FileOwnership};
pub use typescript::emit_functions::emit_ts_function_definitions;
pub use typescript::emit_types::emit_ts_type_definitions;
