/// Errors produced by the constraint/value expression parser.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected character '{0}' at position {1}")]
    UnexpectedChar(char, usize),

    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("expected {0}, got {1}")]
    Expected(String, String),

    #[error("invalid number literal '{0}'")]
    InvalidNumber(String),

    #[error("unterminated string literal")]
    UnterminatedString,

    #[error("unterminated regex pattern")]
    UnterminatedRegex,
}
