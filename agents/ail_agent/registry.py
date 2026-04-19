"""Provider registry stub — real body (discovery + instantiation) lands in task 14.2."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ail_agent.providers.base import LLMProvider


def get_provider(model_spec: str) -> "LLMProvider":
    """Return an ``LLMProvider`` instance for the given model specification.

    ``model_spec`` format: ``"<provider>:<model-name>"``, e.g.
    ``"anthropic:claude-sonnet-4"``.

    Responsibilities (task 14.2):
    - Parse ``model_spec`` and look up the matching provider class.
    - Instantiate the provider (reading credentials from environment, never
      from module-level constants).
    - Raise ``AIL-G0140`` if the provider name is unrecognised.
    """
    raise NotImplementedError("get_provider: task 14.2")
