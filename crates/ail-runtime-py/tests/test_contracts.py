"""Tests for pre(), post(), keep() contract helpers."""

import pytest

from ail_runtime import ContractViolation, keep, post, pre


def test_pre_passes_on_true():
    pre(True)  # must not raise


def test_pre_raises_on_false():
    with pytest.raises(ContractViolation, match="pre-condition violated"):
        pre(False)


def test_pre_includes_message():
    with pytest.raises(ContractViolation, match="amount must be positive"):
        pre(False, "amount must be positive")


def test_post_passes_on_true():
    post(True)  # must not raise


def test_post_raises_on_false():
    with pytest.raises(ContractViolation, match="post-condition violated"):
        post(False)


def test_post_includes_message():
    with pytest.raises(ContractViolation, match="balance unchanged"):
        post(False, "balance unchanged")


def test_keep_passes_on_true():
    keep(True)  # must not raise


def test_keep_raises_on_false():
    with pytest.raises(ContractViolation, match="invariant violated"):
        keep(False)


def test_keep_includes_message():
    with pytest.raises(ContractViolation, match="total non-negative"):
        keep(False, "total non-negative")


def test_contract_violation_is_exception():
    assert issubclass(ContractViolation, Exception)
