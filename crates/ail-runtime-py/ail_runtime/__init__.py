"""AIL runtime library for generated Python code."""

from importlib.metadata import PackageNotFoundError, version

try:
    __version__ = version("ail-runtime")
except PackageNotFoundError:
    __version__ = "0.0.0.dev"

del PackageNotFoundError, version

from .contracts import ContractViolation, keep, post, pre
from .repository import AilRepository, AsyncAilRepository
from .types import (
    AilType,
    EmailAddress,
    Identifier,
    NonEmptyText,
    NonNegativeInteger,
    Percentage,
    PositiveAmount,
    PositiveInteger,
)

__all__ = [
    # contracts
    "pre",
    "post",
    "keep",
    "ContractViolation",
    # base type
    "AilType",
    # builtin validators
    "PositiveInteger",
    "NonNegativeInteger",
    "PositiveAmount",
    "Percentage",
    "NonEmptyText",
    "EmailAddress",
    "Identifier",
    # repository
    "AilRepository",
    "AsyncAilRepository",
    # version
    "__version__",
]
