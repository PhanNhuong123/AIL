"""Tests for AilType base class and all builtin validator subclasses."""

import math

import pytest

from ail_runtime import (
    AilType,
    ContractViolation,
    EmailAddress,
    Identifier,
    NonEmptyText,
    NonNegativeInteger,
    Percentage,
    PositiveAmount,
    PositiveInteger,
)


# ---------------------------------------------------------------------------
# AilType base class
# ---------------------------------------------------------------------------


def test_ail_type_is_abstract():
    with pytest.raises(TypeError):
        AilType()  # type: ignore[abstract]


def test_assert_valid_raises_contract_violation():
    with pytest.raises(ContractViolation):
        PositiveInteger.assert_valid(-1)


def test_assert_valid_passes_silently():
    PositiveInteger.assert_valid(5)  # must not raise


def test_assert_valid_message_contains_class_name():
    with pytest.raises(ContractViolation, match="PositiveInteger"):
        PositiveInteger.assert_valid(-1)


# ---------------------------------------------------------------------------
# PositiveInteger
# ---------------------------------------------------------------------------


def test_positive_integer_valid():
    assert PositiveInteger.validate(1) is True
    assert PositiveInteger.validate(100) is True


def test_positive_integer_zero_invalid():
    assert PositiveInteger.validate(0) is False


def test_positive_integer_negative_invalid():
    assert PositiveInteger.validate(-1) is False


def test_positive_integer_float_whole_invalid():
    # Strict policy: float is rejected even if mathematically whole.
    assert PositiveInteger.validate(5.0) is False


def test_positive_integer_bool_invalid():
    # bool is a subclass of int in Python; excluded by policy.
    assert PositiveInteger.validate(True) is False
    assert PositiveInteger.validate(False) is False


def test_positive_integer_wrong_type_returns_false():
    assert PositiveInteger.validate("hello") is False
    assert PositiveInteger.validate(None) is False


# ---------------------------------------------------------------------------
# NonNegativeInteger
# ---------------------------------------------------------------------------


def test_non_negative_integer_zero_valid():
    assert NonNegativeInteger.validate(0) is True


def test_non_negative_integer_positive_valid():
    assert NonNegativeInteger.validate(42) is True


def test_non_negative_integer_negative_invalid():
    assert NonNegativeInteger.validate(-1) is False


def test_non_negative_integer_float_invalid():
    assert NonNegativeInteger.validate(0.0) is False


def test_non_negative_integer_bool_invalid():
    assert NonNegativeInteger.validate(True) is False


# ---------------------------------------------------------------------------
# PositiveAmount
# ---------------------------------------------------------------------------


def test_positive_amount_int_valid():
    assert PositiveAmount.validate(1) is True


def test_positive_amount_float_valid():
    assert PositiveAmount.validate(0.01) is True
    assert PositiveAmount.validate(99.99) is True


def test_positive_amount_zero_invalid():
    assert PositiveAmount.validate(0) is False
    assert PositiveAmount.validate(0.0) is False


def test_positive_amount_negative_invalid():
    assert PositiveAmount.validate(-1) is False
    assert PositiveAmount.validate(-0.001) is False


def test_positive_amount_nan_invalid():
    assert PositiveAmount.validate(math.nan) is False


def test_positive_amount_inf_invalid():
    assert PositiveAmount.validate(math.inf) is False
    assert PositiveAmount.validate(-math.inf) is False


def test_positive_amount_bool_invalid():
    assert PositiveAmount.validate(True) is False


def test_positive_amount_wrong_type_returns_false():
    assert PositiveAmount.validate("1.5") is False


# ---------------------------------------------------------------------------
# Percentage
# ---------------------------------------------------------------------------


def test_percentage_zero_valid():
    assert Percentage.validate(0) is True
    assert Percentage.validate(0.0) is True


def test_percentage_hundred_valid():
    assert Percentage.validate(100) is True
    assert Percentage.validate(100.0) is True


def test_percentage_midpoint_valid():
    assert Percentage.validate(50) is True
    assert Percentage.validate(50.5) is True


def test_percentage_below_zero_invalid():
    assert Percentage.validate(-1) is False
    assert Percentage.validate(-0.001) is False


def test_percentage_above_hundred_invalid():
    assert Percentage.validate(101) is False
    assert Percentage.validate(100.001) is False


def test_percentage_nan_invalid():
    assert Percentage.validate(math.nan) is False


def test_percentage_inf_invalid():
    assert Percentage.validate(math.inf) is False


def test_percentage_bool_invalid():
    assert Percentage.validate(True) is False


# ---------------------------------------------------------------------------
# NonEmptyText
# ---------------------------------------------------------------------------


def test_non_empty_text_valid():
    assert NonEmptyText.validate("hello") is True
    assert NonEmptyText.validate("  a  ") is True


def test_non_empty_text_empty_invalid():
    assert NonEmptyText.validate("") is False


def test_non_empty_text_whitespace_invalid():
    assert NonEmptyText.validate("   ") is False
    assert NonEmptyText.validate("\t\n") is False


def test_non_empty_text_wrong_type_returns_false():
    assert NonEmptyText.validate(42) is False
    assert NonEmptyText.validate(None) is False


# ---------------------------------------------------------------------------
# EmailAddress
# ---------------------------------------------------------------------------


def test_email_address_valid():
    assert EmailAddress.validate("a@b.com") is True
    assert EmailAddress.validate("user.name+tag@example.co.uk") is True


def test_email_address_no_at_invalid():
    assert EmailAddress.validate("not-an-email") is False


def test_email_address_no_dot_invalid():
    assert EmailAddress.validate("a@b") is False


def test_email_address_whitespace_invalid():
    assert EmailAddress.validate("a b@c.com") is False


def test_email_address_wrong_type_returns_false():
    assert EmailAddress.validate(None) is False


# ---------------------------------------------------------------------------
# Identifier
# ---------------------------------------------------------------------------


def test_identifier_valid():
    assert Identifier.validate("snake_case_123") is True
    assert Identifier.validate("_private") is True
    assert Identifier.validate("CamelCase") is True


def test_identifier_starts_with_digit_invalid():
    assert Identifier.validate("123bad") is False


def test_identifier_hyphen_invalid():
    assert Identifier.validate("kebab-case") is False


def test_identifier_space_invalid():
    assert Identifier.validate("has space") is False


def test_identifier_empty_invalid():
    assert Identifier.validate("") is False


def test_identifier_wrong_type_returns_false():
    assert Identifier.validate(42) is False
