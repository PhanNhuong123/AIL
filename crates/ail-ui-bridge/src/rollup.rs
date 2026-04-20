use crate::types::Status;

/// Compute the worst-child status from a slice of child statuses.
///
/// Returns `Status::Ok` for an empty slice. Variant order (`Ok < Warn < Fail`)
/// is encoded in the `Ord` derivation on `Status`, so `.max()` gives the
/// worst element.
pub fn rollup(children: &[Status]) -> Status {
    children.iter().copied().max().unwrap_or(Status::Ok)
}

/// Compute the status for a function based on whether any contract failed.
pub fn rollup_from_contracts(has_failure: bool) -> Status {
    if has_failure {
        Status::Fail
    } else {
        Status::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Status::{Fail, Ok, Warn};

    #[test]
    fn test_rollup_empty_is_ok() {
        assert_eq!(rollup(&[]), Ok);
    }

    #[test]
    fn test_rollup_single_ok() {
        assert_eq!(rollup(&[Ok]), Ok);
    }

    #[test]
    fn test_rollup_single_warn() {
        assert_eq!(rollup(&[Warn]), Warn);
    }

    #[test]
    fn test_rollup_single_fail() {
        assert_eq!(rollup(&[Fail]), Fail);
    }

    #[test]
    fn test_rollup_ok_warn_is_warn() {
        assert_eq!(rollup(&[Ok, Warn]), Warn);
    }

    #[test]
    fn test_rollup_ok_fail_is_fail() {
        assert_eq!(rollup(&[Ok, Fail]), Fail);
    }

    #[test]
    fn test_rollup_warn_fail_is_fail() {
        assert_eq!(rollup(&[Warn, Fail]), Fail);
    }

    #[test]
    fn test_rollup_all_three_is_fail() {
        assert_eq!(rollup(&[Ok, Warn, Fail]), Fail);
    }

    #[test]
    fn test_rollup_all_ok_is_ok() {
        assert_eq!(rollup(&[Ok, Ok, Ok]), Ok);
    }

    #[test]
    fn test_rollup_all_warn_is_warn() {
        assert_eq!(rollup(&[Warn, Warn]), Warn);
    }

    #[test]
    fn test_rollup_all_fail_is_fail() {
        assert_eq!(rollup(&[Fail, Fail]), Fail);
    }

    #[test]
    fn test_rollup_from_contracts_ok() {
        assert_eq!(rollup_from_contracts(false), Ok);
    }

    #[test]
    fn test_rollup_from_contracts_fail() {
        assert_eq!(rollup_from_contracts(true), Fail);
    }
}
