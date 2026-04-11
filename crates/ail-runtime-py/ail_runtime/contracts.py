"""Contract enforcement functions for AIL-generated code."""

from typing import Any


def pre(condition: bool, message: str = "") -> None:
    """Assert a pre-condition. Raises ContractViolation if false."""
    if not condition:
        raise ContractViolation(f"pre-condition violated: {message}")


def post(condition: bool, message: str = "") -> None:
    """Assert a post-condition. Raises ContractViolation if false."""
    if not condition:
        raise ContractViolation(f"post-condition violated: {message}")


def keep(condition: bool, message: str = "") -> None:
    """Assert an invariant. Raises ContractViolation if false."""
    if not condition:
        raise ContractViolation(f"invariant violated: {message}")


class ContractViolation(Exception):
    """Raised when an AIL contract is violated. Non-catchable by design."""
