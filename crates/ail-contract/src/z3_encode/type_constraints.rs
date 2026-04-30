use ail_types::BuiltinSemanticType;
use z3::ast::{Bool, Dynamic, Int, Real};

use crate::errors::EncodeError;

/// Encode the Z3 assertion(s) that correspond to a [`BuiltinSemanticType`] constraint.
///
/// The returned list is empty only if the type carries no numeric constraint (this
/// should not occur for any current variant). Callers pass all returned `Bool`s to
/// `Solver::assert` as background facts before encoding the actual contract expression.
///
/// # Text types
/// `NonEmptyText`, `EmailAddress`, and `Identifier` require string reasoning that Z3
/// does not support in v0.1. These return [`EncodeError::UnsupportedConstraint`] with
/// variant `"type-text"`. Use runtime `validate_value` checks for these types instead.
///
/// # Sort requirements
/// - **Integer types** (`PositiveInteger`, `NonNegativeInteger`): `var` must be an Int.
/// - **Amount/Percentage**: `var` must be Int or Real; it is promoted to Real internally.
pub fn encode_type_constraint(
    builtin: BuiltinSemanticType,
    var: &Dynamic,
) -> Result<Vec<Bool>, EncodeError> {
    match builtin {
        // PositiveInteger: value > 0  (Int only)
        BuiltinSemanticType::PositiveInteger => {
            let v = require_int(var)?;
            let zero = Int::from_i64(0);
            Ok(vec![v.gt(&zero)])
        }

        // NonNegativeInteger: value >= 0  (Int only)
        BuiltinSemanticType::NonNegativeInteger => {
            let v = require_int(var)?;
            let zero = Int::from_i64(0);
            Ok(vec![v.ge(&zero)])
        }

        // PositiveAmount: value > 0  (Real; Int promoted)
        BuiltinSemanticType::PositiveAmount => {
            let v = require_real_or_promote_int(var)?;
            let zero = Real::from_rational(0, 1);
            Ok(vec![v.gt(&zero)])
        }

        // Percentage: 0 <= value <= 100  (Real; Int promoted)
        BuiltinSemanticType::Percentage => {
            let v = require_real_or_promote_int(var)?;
            let zero = Real::from_rational(0, 1);
            let hundred = Real::from_rational(100, 1);
            Ok(vec![v.ge(&zero), v.le(&hundred)])
        }

        // Text-based types: not encodable in Z3 v0.1
        BuiltinSemanticType::NonEmptyText
        | BuiltinSemanticType::EmailAddress
        | BuiltinSemanticType::Identifier => Err(EncodeError::UnsupportedConstraint {
            variant: "type-text",
        }),
    }
}

/// Extract a Z3 `Int` from a `Dynamic`, or return a sort mismatch error.
fn require_int(var: &Dynamic) -> Result<Int, EncodeError> {
    var.as_int().ok_or(EncodeError::SortMismatch {
        expected: "Int",
        found: "non-Int",
    })
}

/// Extract a Z3 `Real` from a `Dynamic`, promoting an `Int` to `Real` if needed.
///
/// Returns a sort mismatch error for Bool-sorted variables.
fn require_real_or_promote_int(var: &Dynamic) -> Result<Real, EncodeError> {
    if let Some(r) = var.as_real() {
        Ok(r)
    } else if let Some(i) = var.as_int() {
        Ok(i.to_real())
    } else {
        Err(EncodeError::SortMismatch {
            expected: "Real",
            found: "Bool",
        })
    }
}
