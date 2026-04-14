mod assembler;
pub(crate) mod directory;
mod walker;

use pest::Parser;

use crate::errors::ParseError;
use crate::grammar::{AilParser, Rule};

pub use directory::parse_directory;

/// Parse `.ail` source text into an `AilGraph`.
///
/// The returned graph is NOT validated — callers should pass it through
/// `ail_graph::validate_graph()` for structural checks.
pub fn parse(source: &str) -> Result<ail_graph::AilGraph, ParseError> {
    let pairs = AilParser::parse(Rule::file, source).map_err(|e| ParseError::SyntaxError {
        message: e.to_string(),
        span: None,
    })?;
    let statements = walker::walk_file(pairs)?;
    assembler::assemble_graph(statements)
}
