//! CLI error type for `ail`.
//!
//! [`CliError`] is a [`miette::Diagnostic`]-annotated error enum. All command
//! handlers return `Result<(), CliError>`.  The `main` function converts errors
//! into a rich miette report before printing to stderr.

use miette::Diagnostic;

/// Errors that can occur during any CLI command.
#[derive(Debug, Diagnostic, thiserror::Error)]
pub enum CliError {
    /// One or more pipeline stages (parse / validate / type-check / verify) failed.
    #[error("Pipeline errors:\n{errors}")]
    Pipeline { errors: String },

    /// The Python emitter returned errors.
    #[error("Emit errors:\n{errors}")]
    Emit { errors: String },

    /// An I/O operation failed (directory creation, file write, etc.).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The value supplied to `--contracts` is not recognised.
    #[error("Invalid --contracts value `{value}`. Expected: on, comments, off")]
    #[diagnostic(help("Use --contracts on (default), --contracts comments, or --contracts off"))]
    InvalidContracts { value: String },

    /// The command is defined but not yet implemented in this release.
    #[error("`{feature}` is not yet implemented in this release")]
    #[diagnostic(help("This feature is planned for a future phase."))]
    NotImplemented { feature: &'static str },

    /// A required external tool (e.g. pytest) was not found.
    #[error("{message}")]
    #[diagnostic(help("{hint}"))]
    MissingTool { message: String, hint: String },
}
