"""Base type validators for AIL builtin types."""

import math
import re
from abc import ABC, abstractmethod
from typing import Any

from .contracts import ContractViolation

_EMAIL_RE = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")
_IDENTIFIER_RE = re.compile(r"^[a-zA-Z_][a-zA-Z0-9_]*$")


class AilType(ABC):
    """Base class for all AIL-defined types."""

    @classmethod
    @abstractmethod
    def validate(cls, value: Any) -> bool:
        """Return True if value satisfies this type's constraints."""
        ...

    @classmethod
    def assert_valid(cls, value: Any) -> None:
        """Raise ContractViolation if value does not satisfy this type's constraints."""
        if not cls.validate(value):
            raise ContractViolation(f"{value!r} is not a valid {cls.__name__}")


class PositiveInteger(AilType):
    """An integer strictly greater than zero: value > 0.

    Strict: float values are rejected even if they are whole numbers (e.g. 5.0).
    bool is excluded because bool is a subclass of int in Python.
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if not isinstance(value, int) or isinstance(value, bool):
            return False
        return value > 0


class NonNegativeInteger(AilType):
    """An integer greater than or equal to zero: value >= 0.

    Strict: float values and bool are excluded (same policy as PositiveInteger).
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if not isinstance(value, int) or isinstance(value, bool):
            return False
        return value >= 0


class PositiveAmount(AilType):
    """A number (int or float) strictly greater than zero: value > 0.

    Accepts both int and float; must be finite.
    NaN and infinity are rejected.
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if isinstance(value, bool):
            return False
        if isinstance(value, int):
            return value > 0
        if isinstance(value, float):
            return math.isfinite(value) and value > 0.0
        return False


class Percentage(AilType):
    """A number (int or float) in the closed interval [0, 100].

    Accepts both int and float; must be finite.
    NaN and infinity are rejected.
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if isinstance(value, bool):
            return False
        if isinstance(value, int):
            return 0 <= value <= 100
        if isinstance(value, float):
            return math.isfinite(value) and 0.0 <= value <= 100.0
        return False


class NonEmptyText(AilType):
    """A non-empty, non-whitespace-only text string: len(value.strip()) > 0."""

    @classmethod
    def validate(cls, value: Any) -> bool:
        if not isinstance(value, str):
            return False
        return len(value.strip()) > 0


class EmailAddress(AilType):
    """A text string that matches a basic email format: local@domain.tld.

    Pattern: ^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$
    Matches the Rust BuiltinSemanticType::EmailAddress definition exactly.
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if not isinstance(value, str):
            return False
        return bool(_EMAIL_RE.match(value))


class Identifier(AilType):
    """A text string that is a valid AIL identifier: ^[a-zA-Z_][a-zA-Z0-9_]*$.

    Matches the Rust BuiltinSemanticType::Identifier definition exactly.
    """

    @classmethod
    def validate(cls, value: Any) -> bool:
        if not isinstance(value, str):
            return False
        return bool(_IDENTIFIER_RE.match(value))
