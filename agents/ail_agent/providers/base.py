"""LLMProvider Protocol — the uniform interface every provider must satisfy."""

from __future__ import annotations

from typing import runtime_checkable

from typing_extensions import Protocol


@runtime_checkable
class LLMProvider(Protocol):
    """Uniform interface for LLM provider adapters.

    Implementations are transport-agnostic.  They must not import from
    ``langgraph`` or the orchestrator.  API credentials must be read at
    call time — never as module-level constants.
    """

    name: str

    def complete(self, system: str, user: str, *, model: str) -> str:
        """Send a prompt and return the model's text response.

        Parameters
        ----------
        system:
            System/instruction prompt.
        user:
            User-turn message.
        model:
            Provider-specific model identifier (e.g. ``"claude-sonnet-4"``).
        """
        ...
