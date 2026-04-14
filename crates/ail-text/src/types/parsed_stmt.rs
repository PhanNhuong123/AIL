use ail_graph::{Contract, Expression, NodeMetadata, Pattern};

use crate::types::SourceSpan;

/// A single statement extracted from the pest parse tree.
///
/// This is the intermediate representation between the walker (which reads pest
/// pairs) and the assembler (which builds `AilGraph`). It captures everything
/// the assembler needs without requiring knowledge of pest types.
#[derive(Debug, Clone)]
pub struct ParsedStatement {
    /// Indentation depth in spaces (0, 2, 4, ...). Divide by 2 for logical depth.
    pub indent: usize,
    /// The canonical AIL pattern for this statement.
    pub pattern: Pattern,
    /// Human-readable description of what this statement does.
    pub intent: String,
    /// Pre-populated metadata (name, params, fields, return_type, etc.).
    pub metadata: NodeMetadata,
    /// Expression text, if this is a leaf-level action.
    pub expression: Option<Expression>,
    /// Contracts — populated only for `Pattern::Promise` statements.
    pub contracts: Vec<Contract>,
    /// Source location for error reporting.
    pub span: SourceSpan,
    /// Inline child statements — populated only for `Together` and `Retry`.
    pub inline_children: Vec<ParsedStatement>,
}
