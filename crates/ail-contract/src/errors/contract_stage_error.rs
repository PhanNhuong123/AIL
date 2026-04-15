use thiserror::Error;

use super::ContractError;

/// Unified error type for the [`crate::verify`] pipeline entry point.
///
/// Both static contract scope errors and Z3 verification errors are reported
/// through this enum, allowing downstream callers to handle a single error
/// type rather than two separate collections.
///
/// Static check errors (variant [`ContractStageError::StaticCheck`]) are
/// always reported. Z3 verification errors (variant
/// `Z3Verify`) are only produced when the `z3-verify`
/// feature is enabled.
#[derive(Debug, Clone, Error)]
pub enum ContractStageError {
    /// A static contract scope error detected before Z3 verification.
    ///
    /// Static checks always run. When any static errors are found, Z3
    /// verification is skipped entirely.
    #[error("{0}")]
    StaticCheck(ContractError),

    /// A Z3 contract verification error (only with the `z3-verify` feature).
    ///
    /// Z3 errors are only produced when all static checks have passed.
    #[cfg(feature = "z3-verify")]
    #[error("{0}")]
    Z3Verify(super::VerifyError),
}
