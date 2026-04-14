use pest_derive::Parser;

/// Pest-generated parser for the AIL text format.
///
/// The grammar is defined in `grammar.pest` (same directory).
/// The `Rule` enum is auto-generated from that file.
#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct AilParser;
