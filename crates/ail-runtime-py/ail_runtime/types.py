"""Base type validators for AIL builtin types."""

from abc import ABC, abstractmethod
from typing import Any


class AilType(ABC):
    """Base class for all AIL-defined types."""

    @classmethod
    @abstractmethod
    def validate(cls, value: Any) -> bool:
        """Return True if value satisfies this type's constraints."""
        ...

    @classmethod
    def assert_valid(cls, value: Any) -> None:
        if not cls.validate(value):
            raise TypeError(f"{value!r} is not a valid {cls.__name__}")
