//! `ail-text` — pest-based `.ail` parser and deterministic renderer.
//!
//! This crate owns the text-facing layer of the AIL compiler pipeline:
//! - A PEG grammar covering all 17 AIL patterns ([`Rule`]).
//! - A parser that converts `.ail` text into graph-compatible structures ([`parse`], [`parse_directory`]).
//! - A deterministic renderer that converts graph state back to `.ail` text ([`render`]).
//!
//! ## Pipeline position
//!
//! ```text
//! .ail files → parse() → AilGraph node data → (ail-graph validate)
//! TypedGraph → render() → .ail text
//! ```
//!
//! ## Entry points
//!
//! - [`parse`] — parse a single `.ail` source string.
//! - [`parse_directory`] — parse all `.ail` files in a project directory.
//! - [`render`] — render a graph node to `.ail` text at a given depth.

pub mod errors;
pub mod grammar;
pub mod parser;
pub mod renderer;
pub mod types;

pub use errors::ParseError;
pub use grammar::{AilParser, Rule};
pub use parser::{parse, parse_directory};
pub use renderer::render;
